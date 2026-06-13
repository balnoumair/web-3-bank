//! Per-chain withdrawal routing for `GetWithdrawalRouting`.
//!
//! Composes live `balanceOf`, `reserveDepth`, and RouteReceiver activation
//! state into withdrawable amounts per chain.

use std::collections::HashMap;
use std::sync::Arc;

use alloy_primitives::{keccak256, U256};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::account_balance::read_chain_balance;
use crate::config::Config;
use crate::domain::newtypes::ChainId;
use crate::domain::repository::AccountEventRepository;
use crate::domain::withdrawal_routing::{
    compute_withdrawal_routing, ChainWithdrawalInput,
};
use crate::eth;
use crate::hot_path::HotPath;
use crate::proto::treasury::{
    GetWithdrawalRoutingRequest, GetWithdrawalRoutingResponse, WithdrawalRoutingEntry,
};

pub struct AccountWithdrawalRoutingService {
    config: Arc<Config>,
    http: reqwest::Client,
    account_events: Arc<dyn AccountEventRepository>,
    hot_path: Arc<HotPath>,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
    reserve_depth_selector: [u8; 4],
    sync_usd_cache: Arc<Mutex<HashMap<ChainId, String>>>,
}

impl AccountWithdrawalRoutingService {
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
        let inputs = self.gather_chain_inputs(&address).await;
        let computed = compute_withdrawal_routing(&inputs);

        let entries = computed
            .into_iter()
            .map(|entry| WithdrawalRoutingEntry {
                chain_id: entry.chain_id,
                withdrawable_wei: entry.withdrawable_wei.to_string(),
                available: entry.available,
                reason: entry.reason.unwrap_or_default(),
                balance_wei: entry.balance_wei.to_string(),
            })
            .collect();

        Ok(Response::new(GetWithdrawalRoutingResponse { entries }))
    }

    async fn gather_chain_inputs(&self, address: &str) -> Vec<ChainWithdrawalInput> {
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
            let http = self.http.clone();
            let account_events = Arc::clone(&self.account_events);
            let hot_path = Arc::clone(&self.hot_path);
            let sync_usd_cache = Arc::clone(&self.sync_usd_cache);
            let balance_of_selector = self.balance_of_selector;
            let sync_usd_selector = self.sync_usd_selector;
            let reserve_depth_selector = self.reserve_depth_selector;
            let address = address.to_string();

            tasks.push(tokio::spawn(async move {
                read_chain_withdrawal_input(
                    http,
                    account_events,
                    hot_path,
                    sync_usd_cache,
                    chain_id,
                    rpc_url,
                    bank_addr,
                    address,
                    balance_of_selector,
                    sync_usd_selector,
                    reserve_depth_selector,
                )
                .await
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
        inputs
    }
}

async fn read_chain_withdrawal_input(
    http: reqwest::Client,
    account_events: Arc<dyn AccountEventRepository>,
    hot_path: Arc<HotPath>,
    sync_usd_cache: Arc<Mutex<HashMap<ChainId, String>>>,
    chain_id: ChainId,
    rpc_url: String,
    bank_addr: String,
    user_address: String,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
    reserve_depth_selector: [u8; 4],
) -> Option<ChainWithdrawalInput> {
    let decommissioned = hot_path.is_chain_decommissioned(chain_id.0).await;
    let active = hot_path.is_chain_active(chain_id.0).await;

    let balance = read_chain_balance(
        http.clone(),
        account_events,
        sync_usd_cache,
        chain_id,
        rpc_url.clone(),
        bank_addr.clone(),
        user_address.clone(),
        balance_of_selector,
        sync_usd_selector,
    )
    .await?;

    let reserve_depth = eth::fetch_reserve_depth(
        &http,
        &rpc_url,
        &bank_addr,
        &reserve_depth_selector,
    )
    .await
    .unwrap_or(U256::ZERO);

    Some(ChainWithdrawalInput {
        chain_id: chain_id.0,
        balance_wei: balance.amount,
        reserve_depth_wei: reserve_depth,
        active,
        decommissioned,
    })
}
