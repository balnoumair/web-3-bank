//! Aggregated cross-chain SyncUSD balance reads for `GetBalance`.
//!
//! Fans out live `balanceOf` RPC calls per non-decommissioned chain, falls
//! back to the indexed event estimate when a chain RPC fails, and caches
//! results briefly to limit RPC load.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use alloy_primitives::{keccak256, U256};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::config::Config;
use crate::domain::balance::{aggregate_balances, ChainBalanceContribution};
use crate::domain::newtypes::ChainId;
use crate::domain::repository::AccountEventRepository;
use crate::eth;
use crate::hot_path::HotPath;
use crate::proto::treasury::{GetBalanceRequest, GetBalanceResponse};

const BALANCE_CACHE_TTL: Duration = Duration::from_secs(4);

struct CachedBalance {
    balance_wei: String,
    degraded: bool,
    fetched_at: Instant,
}

pub struct AccountBalanceService {
    config: Arc<Config>,
    http: reqwest::Client,
    account_events: Arc<dyn AccountEventRepository>,
    hot_path: Arc<HotPath>,
    cache: Mutex<HashMap<String, CachedBalance>>,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
    sync_usd_cache: Arc<Mutex<HashMap<ChainId, String>>>,
}

impl AccountBalanceService {
    pub fn new(
        config: Arc<Config>,
        http: reqwest::Client,
        account_events: Arc<dyn AccountEventRepository>,
        hot_path: Arc<HotPath>,
    ) -> Arc<Self> {
        let balance_of_hash = keccak256(b"balanceOf(address)");
        let sync_usd_hash = keccak256(b"syncUSD()");
        let mut balance_of_selector = [0u8; 4];
        balance_of_selector.copy_from_slice(&balance_of_hash[..4]);
        let mut sync_usd_selector = [0u8; 4];
        sync_usd_selector.copy_from_slice(&sync_usd_hash[..4]);

        Arc::new(Self {
            config,
            http,
            account_events,
            hot_path,
            cache: Mutex::new(HashMap::new()),
            balance_of_selector,
            sync_usd_selector,
            sync_usd_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn get_balance(
        &self,
        req: Request<GetBalanceRequest>,
    ) -> Result<Response<GetBalanceResponse>, Status> {
        let address = req.into_inner().address;
        let key = address.to_lowercase();

        {
            let cache = self.cache.lock().await;
            if let Some(entry) = cache.get(&key) {
                if entry.fetched_at.elapsed() < BALANCE_CACHE_TTL {
                    return Ok(Response::new(GetBalanceResponse {
                        balance_wei: entry.balance_wei.clone(),
                        degraded: entry.degraded,
                    }));
                }
            }
        }

        let (balance_wei, degraded) = self.fetch_aggregated_balance(&address).await;

        self.cache.lock().await.insert(
            key,
            CachedBalance {
                balance_wei: balance_wei.clone(),
                degraded,
                fetched_at: Instant::now(),
            },
        );

        Ok(Response::new(GetBalanceResponse {
            balance_wei,
            degraded,
        }))
    }

    async fn fetch_aggregated_balance(&self, address: &str) -> (String, bool) {
        let chains: Vec<(ChainId, String, String)> = self
            .config
            .rpc_urls
            .iter()
            .filter_map(|(&chain_id, rpc_url)| {
                self.config
                    .contract_addresses
                    .get(&chain_id)
                    .map(|bank| (ChainId(chain_id), rpc_url.clone(), bank.clone()))
            })
            .collect();

        let mut tasks = Vec::new();
        for (chain_id, rpc_url, bank_addr) in chains {
            if self.hot_path.is_chain_decommissioned(chain_id.0).await {
                continue;
            }

            let http = self.http.clone();
            let account_events = Arc::clone(&self.account_events);
            let sync_usd_cache = Arc::clone(&self.sync_usd_cache);
            let balance_of_selector = self.balance_of_selector;
            let sync_usd_selector = self.sync_usd_selector;
            let address = address.to_string();
            let bank_addr = bank_addr.clone();

            tasks.push(tokio::spawn(async move {
                read_chain_balance(
                    http,
                    account_events,
                    sync_usd_cache,
                    chain_id,
                    rpc_url,
                    bank_addr,
                    address,
                    balance_of_selector,
                    sync_usd_selector,
                )
                .await
            }));
        }

        let mut contributions = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Some(c)) => contributions.push(c),
                Ok(None) => {}
                Err(e) => warn!(err = %e, "account_balance: chain task join failed"),
            }
        }

        let (total, degraded) = aggregate_balances(&contributions);
        (total.to_string(), degraded)
    }
}

pub(crate) async fn read_chain_balance(
    http: reqwest::Client,
    account_events: Arc<dyn AccountEventRepository>,
    sync_usd_cache: Arc<Mutex<HashMap<ChainId, String>>>,
    chain_id: ChainId,
    rpc_url: String,
    bank_addr: String,
    user_address: String,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
) -> Option<ChainBalanceContribution> {
    let token_addr = {
        let cache = sync_usd_cache.lock().await;
        if let Some(addr) = cache.get(&chain_id) {
            addr.clone()
        } else {
            drop(cache);
            let addr =
                eth::fetch_address_view(&http, &rpc_url, &bank_addr, &sync_usd_selector).await?;
            sync_usd_cache.lock().await.insert(chain_id, addr.clone());
            addr
        }
    };

    if let Some(live) = eth::fetch_balance_of(
        &http,
        &rpc_url,
        &token_addr,
        &user_address,
        &balance_of_selector,
    )
    .await
    {
        return Some(ChainBalanceContribution {
            amount: live,
            used_fallback: false,
        });
    }

    warn!(
        chain_id = chain_id.0,
        "account_balance: live balanceOf failed — using indexed fallback"
    );
    let indexed = account_events
        .indexed_balance_on_chain(chain_id, &user_address)
        .await;
    let amount = indexed.parse::<U256>().unwrap_or(U256::ZERO);
    Some(ChainBalanceContribution {
        amount,
        used_fallback: true,
    })
}
