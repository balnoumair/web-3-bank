//! `GetWithdrawalRouting` — per-chain withdrawability for the BFF / client.
//!
//! Composes live `balanceOf`, `reserveDepth()`, and RouteReceiver activation
//! state. Decommissioned chains are omitted entirely.

use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::{keccak256, U256};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::config::Config;
use crate::domain::newtypes::ChainId;
use crate::domain::repository::AccountEventRepository;
use crate::domain::withdrawal_routing::{
    compute_withdrawal_routing, ChainWithdrawalInput, ChainWithdrawalEntry,
};
use crate::eth;
use crate::hot_path::HotPath;
use crate::proto::treasury::{
    GetWithdrawalRoutingRequest, GetWithdrawalRoutingResponse, WithdrawalRoutingEntry,
};

pub struct WithdrawalRoutingService {
    config: Arc<Config>,
    http: reqwest::Client,
    account_events: Arc<dyn AccountEventRepository>,
    hot_path: Arc<HotPath>,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
    reserve_depth_selector: [u8; 4],
    sync_usd_cache: Arc<Mutex<HashMap<ChainId, String>>>,
}

impl WithdrawalRoutingService {
    pub fn new(
        config: Arc<Config>,
        http: reqwest::Client,
        account_events: Arc<dyn AccountEventRepository>,
        hot_path: Arc<HotPath>,
    ) -> Arc<Self> {
        let balance_of_hash = keccak256(b"balanceOf(address)");
        let sync_usd_hash = keccak256(b"syncUSD()");
        let reserve_depth_hash = keccak256(b"reserveDepth()");
        let mut balance_of_selector = [0u8; 4];
        balance_of_selector.copy_from_slice(&balance_of_hash[..4]);
        let mut sync_usd_selector = [0u8; 4];
        sync_usd_selector.copy_from_slice(&sync_usd_hash[..4]);
        let mut reserve_depth_selector = [0u8; 4];
        reserve_depth_selector.copy_from_slice(&reserve_depth_hash[..4]);

        Arc::new(Self {
            config,
            http,
            account_events,
            hot_path,
            balance_of_selector,
            sync_usd_selector,
            reserve_depth_selector,
            sync_usd_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn get_withdrawal_routing(
        &self,
        req: Request<GetWithdrawalRoutingRequest>,
    ) -> Result<Response<GetWithdrawalRoutingResponse>, Status> {
        let address = req.into_inner().address;
        let entries = self.fetch_routing(&address).await;
        Ok(Response::new(GetWithdrawalRoutingResponse {
            entries: entries.into_iter().map(to_proto_entry).collect(),
        }))
    }

    async fn fetch_routing(&self, address: &str) -> Vec<ChainWithdrawalEntry> {
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
            let hot_path = Arc::clone(&self.hot_path);
            let balance_of_selector = self.balance_of_selector;
            let sync_usd_selector = self.sync_usd_selector;
            let reserve_depth_selector = self.reserve_depth_selector;
            let address = address.to_string();
            let bank_addr = bank_addr.clone();

            tasks.push(tokio::spawn(async move {
                let active = hot_path.is_chain_active(chain_id.0).await;
                let decommissioned = hot_path.is_chain_decommissioned(chain_id.0).await;

                let balance_wei =
                    read_chain_balance_wei(
                        http.clone(),
                        account_events,
                        sync_usd_cache,
                        chain_id,
                        rpc_url.clone(),
                        bank_addr.clone(),
                        address.clone(),
                        balance_of_selector,
                        sync_usd_selector,
                    )
                    .await?;

                let reserve_depth_wei = eth::fetch_reserve_depth(
                    &http,
                    &rpc_url,
                    &bank_addr,
                    &reserve_depth_selector,
                )
                .await
                .unwrap_or(U256::ZERO);

                Some(ChainWithdrawalInput {
                    chain_id: chain_id.0,
                    balance_wei,
                    reserve_depth_wei,
                    active,
                    decommissioned,
                })
            }));
        }

        let mut inputs = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Some(input)) => inputs.push(input),
                Ok(None) => {}
                Err(e) => warn!(err = %e, "withdrawal_routing: chain task join failed"),
            }
        }

        compute_withdrawal_routing(&inputs)
    }
}

fn to_proto_entry(entry: ChainWithdrawalEntry) -> WithdrawalRoutingEntry {
    WithdrawalRoutingEntry {
        chain_id: entry.chain_id,
        withdrawable_wei: entry.withdrawable_wei.to_string(),
        available: entry.available,
        reason: entry.reason,
        balance_wei: entry.balance_wei.to_string(),
    }
}

async fn read_chain_balance_wei(
    http: reqwest::Client,
    account_events: Arc<dyn AccountEventRepository>,
    sync_usd_cache: Arc<Mutex<HashMap<ChainId, String>>>,
    chain_id: ChainId,
    rpc_url: String,
    bank_addr: String,
    user_address: String,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
) -> Option<U256> {
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
        return Some(live);
    }

    warn!(
        chain_id = chain_id.0,
        "withdrawal_routing: live balanceOf failed — using indexed fallback"
    );
    let indexed = account_events
        .indexed_balance_on_chain(chain_id, &user_address)
        .await;
    Some(indexed.parse::<U256>().unwrap_or(U256::ZERO))
}
