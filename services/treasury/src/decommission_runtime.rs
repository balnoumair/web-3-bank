use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, Address, U256};
use async_trait::async_trait;
use k256::ecdsa::SigningKey;
use sqlx::PgPool;
use tonic::{metadata::MetadataValue, Request};
use tracing::{info, warn};

use crate::config::Config;
use crate::decommission::{
    BankDrainPort, BridgeReceipt, ChainStatePort, DrainPlan, HolderBalance, HolderIndexPort,
    OperatorAlertPort, UserHomeChainPort,
};
use crate::domain::abi::{encode_bridge_reserve, encode_rebalance, extract_ccip_message_id};
use crate::domain::newtypes::{ChainId, TxHash};
use crate::eth;
use crate::user_pb::{user_service_client::UserServiceClient, SetUserHomeChainRequest};

pub struct RuntimeHolderIndex {
    cfg: Arc<Config>,
    http: reqwest::Client,
    pool: PgPool,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
}

impl RuntimeHolderIndex {
    pub fn new(cfg: Arc<Config>, http: reqwest::Client, pool: PgPool) -> Self {
        let mut balance_of_selector = [0u8; 4];
        balance_of_selector.copy_from_slice(&keccak256(b"balanceOf(address)")[..4]);
        let mut sync_usd_selector = [0u8; 4];
        sync_usd_selector.copy_from_slice(&keccak256(b"syncUSD()")[..4]);
        Self {
            cfg,
            http,
            pool,
            balance_of_selector,
            sync_usd_selector,
        }
    }

    pub async fn index_fresh_enough(&self, chain: ChainId) -> bool {
        let Some(rpc_url) = self.cfg.rpc_urls.get(&chain.0) else {
            return false;
        };
        let Some(head) = eth::fetch_block_number(&self.http, rpc_url).await else {
            return false;
        };
        let Some(cursor) = sqlx::query_scalar::<_, i64>(
            "SELECT last_block FROM treasury.index_cursors WHERE chain_id = $1",
        )
        .bind(chain.0 as i64)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|v| v as u64) else {
            return false;
        };
        let tolerance = self.cfg.decommission_index_head_tolerance_blocks;
        head.saturating_sub(cursor) <= tolerance
    }

    pub async fn holders_for_chain_index(&self, chain: ChainId) -> Vec<String> {
        let rows = sqlx::query_scalar::<_, Option<String>>(
            "SELECT DISTINCT lower(address_to) AS holder
             FROM treasury.account_events
             WHERE chain_id = $1
               AND event_kind IN ('deposited', 'transfer', 'hot_path_released')
               AND address_to IS NOT NULL",
        )
        .bind(chain.0 as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        rows.into_iter().flatten().collect()
    }
}

#[async_trait]
impl HolderIndexPort for RuntimeHolderIndex {
    async fn holders_for_chain(&self, chain: ChainId) -> Vec<String> {
        self.holders_for_chain_index(chain).await
    }

    async fn balance_of(&self, chain: ChainId, holder: &str) -> U256 {
        let Some(rpc_url) = self.cfg.rpc_urls.get(&chain.0) else {
            return U256::ZERO;
        };
        let Some(bank_addr) = self.cfg.contract_addresses.get(&chain.0) else {
            return U256::ZERO;
        };
        let Some(token_addr) =
            eth::fetch_address_view(&self.http, rpc_url, bank_addr, &self.sync_usd_selector).await
        else {
            return U256::ZERO;
        };
        eth::fetch_balance_of(
            &self.http,
            rpc_url,
            &token_addr,
            holder,
            &self.balance_of_selector,
        )
        .await
        .unwrap_or(U256::ZERO)
    }
}

pub struct RuntimeChainState {
    hot_path: Arc<crate::hot_path::HotPath>,
    cfg: Arc<Config>,
    http: reqwest::Client,
    decommission_selector: [u8; 4],
}

impl RuntimeChainState {
    pub fn new(
        hot_path: Arc<crate::hot_path::HotPath>,
        cfg: Arc<Config>,
        http: reqwest::Client,
    ) -> Self {
        let mut decommission_selector = [0u8; 4];
        decommission_selector
            .copy_from_slice(&keccak256(b"getChainDecommissionStatus(uint256)")[..4]);
        Self {
            hot_path,
            cfg,
            http,
            decommission_selector,
        }
    }

    pub async fn is_source_draining(&self, source_chain: ChainId) -> bool {
        let Some(rpc_url) = self.cfg.rpc_urls.values().next() else {
            return false;
        };
        let call_data = encode_uint_arg_call(&self.decommission_selector, source_chain.0);
        let body = serde_json::json!({
            "jsonrpc":"2.0",
            "method":"eth_call",
            "params":[{"to": self.cfg.route_receiver_address, "data": call_data}, "latest"],
            "id":1
        });
        let Ok(resp) = self.http.post(rpc_url).json(&body).send().await else {
            return false;
        };
        let Ok(json) = resp.json::<serde_json::Value>().await else {
            return false;
        };
        let Some(bytes) = json["result"].as_str().and_then(eth::decode_hex) else {
            return false;
        };
        if bytes.len() < 64 {
            return false;
        }
        let draining = bytes[31] == 1;
        let decommissioned = bytes[63] == 1;
        draining && !decommissioned
    }
}

#[async_trait]
impl ChainStatePort for RuntimeChainState {
    async fn is_chain_active(&self, chain: ChainId) -> bool {
        self.hot_path.is_chain_active(chain.0).await
    }
}

pub struct RuntimeBankDrain {
    cfg: Arc<Config>,
    http: reqwest::Client,
    signer: Arc<SigningKey>,
    signer_address: Address,
    nonce_cache: Arc<tokio::sync::Mutex<std::collections::HashMap<ChainId, u64>>>,
    rebalance_selector: [u8; 4],
    bridge_reserve_selector: [u8; 4],
}

impl RuntimeBankDrain {
    pub fn new(cfg: Arc<Config>, http: reqwest::Client) -> Option<Self> {
        let (signer, signer_address) = eth::load_signing_key(&cfg.relayer_key_path);
        let signer = signer?;
        let signer_address = signer_address?;
        let mut rebalance_selector = [0u8; 4];
        rebalance_selector.copy_from_slice(&keccak256(b"rebalance(uint64,uint256)")[..4]);
        let mut bridge_reserve_selector = [0u8; 4];
        bridge_reserve_selector.copy_from_slice(&keccak256(b"bridgeReserve(uint64,uint256)")[..4]);
        Some(Self {
            cfg,
            http,
            signer,
            signer_address,
            nonce_cache: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            rebalance_selector,
            bridge_reserve_selector,
        })
    }

    async fn submit_bank_call(
        &self,
        source_chain: ChainId,
        data: Vec<u8>,
    ) -> Result<(TxHash, Option<String>), String> {
        let rpc_url = self
            .cfg
            .rpc_urls
            .get(&source_chain.0)
            .ok_or_else(|| "missing source chain rpc".to_string())?;
        let bank_addr = self
            .cfg
            .contract_addresses
            .get(&source_chain.0)
            .ok_or_else(|| "missing source chain bank".to_string())?;
        let nonce = {
            let mut cache = self.nonce_cache.lock().await;
            if let Some(n) = cache.get(&source_chain).copied() {
                n
            } else {
                let fetched = eth::fetch_nonce(&self.http, rpc_url, &self.signer_address)
                    .await
                    .map_err(|e| e.to_string())?;
                cache.insert(source_chain, fetched);
                fetched
            }
        };
        let (max_fee, max_priority_fee) = eth::fetch_gas_params(&self.http, rpc_url)
            .await
            .map_err(|e| e.to_string())?;
        let bank_addr_bytes = eth::decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or_else(|| "invalid bank address".to_string())?;
        let raw_hex = eth::sign_eip1559_tx(
            source_chain.0,
            nonce,
            max_priority_fee,
            max_fee,
            300_000,
            &bank_addr_bytes,
            &[],
            &data,
            &self.signer,
        )
        .map_err(|e| e.to_string())?;
        let tx_hash = eth::send_raw_transaction(&self.http, rpc_url, &raw_hex)
            .await
            .map_err(|e| e.to_string())?;
        let logs =
            eth::wait_for_receipt_logs(&self.http, rpc_url, &tx_hash, Duration::from_secs(90))
                .await
                .map_err(|e| e.to_string())?;
        self.nonce_cache
            .lock()
            .await
            .insert(source_chain, nonce.saturating_add(1));
        let message_id = extract_ccip_message_id(&logs);
        Ok((TxHash(tx_hash), message_id))
    }

    pub async fn has_required_roles(&self, source_chain: ChainId) -> bool {
        let Some(rpc_url) = self.cfg.rpc_urls.get(&source_chain.0) else {
            return false;
        };
        let Some(bank_addr) = self.cfg.contract_addresses.get(&source_chain.0) else {
            return false;
        };
        let reb = has_role(
            &self.http,
            rpc_url,
            bank_addr,
            &keccak256(b"REBALANCER_ROLE"),
            self.signer_address,
        )
        .await;
        let res = has_role(
            &self.http,
            rpc_url,
            bank_addr,
            &keccak256(b"RESERVE_REBALANCER_ROLE"),
            self.signer_address,
        )
        .await;
        reb && res
    }
}

#[async_trait]
impl BankDrainPort for RuntimeBankDrain {
    async fn bridge_holder(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        holder: &str,
        amount: U256,
    ) -> Result<BridgeReceipt, String> {
        let data = encode_rebalance(&self.rebalance_selector, target_chain.0, &amount);
        let (tx_hash, message_id) = self.submit_bank_call(source_chain, data).await?;
        info!(
            holder,
            source_chain = source_chain.0,
            target_chain = target_chain.0,
            amount = %amount,
            "decommission: holder bridge submitted"
        );
        Ok(BridgeReceipt {
            src_message_id: message_id,
            dst_tx_hash: Some(tx_hash),
        })
    }

    async fn drain_pool(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        amount: U256,
    ) -> Result<(), String> {
        let data = encode_rebalance(&self.rebalance_selector, target_chain.0, &amount);
        self.submit_bank_call(source_chain, data).await.map(|_| ())
    }

    async fn drain_reserve(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        amount: U256,
    ) -> Result<(), String> {
        let data = encode_bridge_reserve(&self.bridge_reserve_selector, target_chain.0, &amount);
        self.submit_bank_call(source_chain, data).await.map(|_| ())
    }
}

pub struct RuntimeUserHomeChain {
    cfg: Arc<Config>,
}

impl RuntimeUserHomeChain {
    pub fn new(cfg: Arc<Config>) -> Self {
        Self { cfg }
    }
}

#[async_trait]
impl UserHomeChainPort for RuntimeUserHomeChain {
    async fn set_user_home_chain(&self, holder: &str, target_chain: ChainId) -> Result<(), String> {
        let endpoint = self
            .cfg
            .user_service_addr
            .clone()
            .ok_or_else(|| "USER_SERVICE_ADDR is not configured".to_string())?;
        let uri = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            endpoint
        } else {
            format!("http://{endpoint}")
        };
        let mut client = UserServiceClient::connect(uri)
            .await
            .map_err(|e| e.to_string())?;
        let token = self
            .cfg
            .decommission_orchestrator_token
            .clone()
            .ok_or_else(|| "DECOMMISSION_ORCHESTRATOR_TOKEN is not configured".to_string())?;
        let mut req = Request::new(SetUserHomeChainRequest {
            tempo_address: holder.to_string(),
            chain_id: target_chain.0,
            decommission_override: true,
            operator: "treasury-decommission-orchestrator".to_string(),
        });
        let token_meta = MetadataValue::from_str(&token).map_err(|e| e.to_string())?;
        req.metadata_mut()
            .insert("x-decommission-orchestrator-token", token_meta);
        client
            .set_user_home_chain(req)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub struct RuntimeAlerts;

#[async_trait]
impl OperatorAlertPort for RuntimeAlerts {
    async fn alert(&self, message: &str) {
        warn!(message, "decommission alert");
    }
}

pub async fn build_drain_plan(
    cfg: Arc<Config>,
    http: reqwest::Client,
    source_chain: ChainId,
    target_chain: ChainId,
    holders: Vec<String>,
    balance_of_selector: [u8; 4],
    sync_usd_selector: [u8; 4],
) -> DrainPlan {
    let mut unique = HashSet::new();
    let mut resolved = Vec::new();
    let Some(source_rpc) = cfg.rpc_urls.get(&source_chain.0) else {
        return empty_plan(source_chain, target_chain);
    };
    let Some(source_bank) = cfg.contract_addresses.get(&source_chain.0) else {
        return empty_plan(source_chain, target_chain);
    };
    let Some(sync_usd_addr) =
        eth::fetch_address_view(&http, source_rpc, source_bank, &sync_usd_selector).await
    else {
        return empty_plan(source_chain, target_chain);
    };
    for holder in holders {
        let lowered = holder.to_lowercase();
        if !unique.insert(lowered) {
            continue;
        }
        let amount = eth::fetch_balance_of(
            &http,
            source_rpc,
            &sync_usd_addr,
            &holder,
            &balance_of_selector,
        )
        .await
        .unwrap_or(U256::ZERO);
        if amount.is_zero() {
            continue;
        }
        resolved.push(HolderBalance {
            address: holder,
            amount,
        });
    }
    let mut pool_selector = [0u8; 4];
    pool_selector.copy_from_slice(&keccak256(b"poolDepth()")[..4]);
    let mut reserve_selector = [0u8; 4];
    reserve_selector.copy_from_slice(&keccak256(b"reserveDepth()")[..4]);
    let pool_amount = eth::fetch_pool_depth(&http, source_rpc, source_bank, &pool_selector)
        .await
        .unwrap_or(U256::ZERO);
    let reserve_amount =
        eth::fetch_reserve_depth(&http, source_rpc, source_bank, &reserve_selector)
            .await
            .unwrap_or(U256::ZERO);
    DrainPlan {
        source_chain,
        target_chain,
        holders: resolved,
        pool_amount,
        reserve_amount,
    }
}

fn empty_plan(source_chain: ChainId, target_chain: ChainId) -> DrainPlan {
    DrainPlan {
        source_chain,
        target_chain,
        holders: Vec::new(),
        pool_amount: U256::ZERO,
        reserve_amount: U256::ZERO,
    }
}

fn encode_uint_arg_call(selector: &[u8; 4], value: u64) -> String {
    let mut data = Vec::with_capacity(4 + 32);
    data.extend_from_slice(selector);
    data.extend_from_slice(&[0u8; 24]);
    data.extend_from_slice(&value.to_be_bytes());
    format!("0x{}", eth::bytes_to_hex(&data))
}

async fn has_role(
    http: &reqwest::Client,
    rpc_url: &str,
    bank_addr: &str,
    role_hash: &[u8; 32],
    account: Address,
) -> bool {
    let selector = &keccak256(b"hasRole(bytes32,address)")[..4];
    let mut payload = Vec::with_capacity(4 + 32 + 32);
    payload.extend_from_slice(selector);
    payload.extend_from_slice(role_hash);
    payload.extend_from_slice(&[0u8; 12]);
    payload.extend_from_slice(account.as_slice());
    let body = serde_json::json!({
        "jsonrpc":"2.0",
        "method":"eth_call",
        "params":[{"to": bank_addr, "data": format!("0x{}", eth::bytes_to_hex(&payload))}, "latest"],
        "id":1
    });
    let Ok(resp) = http.post(rpc_url).json(&body).send().await else {
        return false;
    };
    let Ok(json) = resp.json::<serde_json::Value>().await else {
        return false;
    };
    let Some(bytes) = json["result"].as_str().and_then(eth::decode_hex) else {
        return false;
    };
    bytes.len() >= 32 && bytes[31] == 1
}

pub fn parse_drain_id(drain_id: &str) -> Option<(ChainId, ChainId)> {
    let trimmed = drain_id.trim();
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    let src = parts[0].parse::<u64>().ok()?;
    let dst = parts[1].parse::<u64>().ok()?;
    Some((ChainId(src), ChainId(dst)))
}

pub fn make_drain_id(source_chain: ChainId, target_chain: ChainId) -> String {
    format!("{}-{}", source_chain.0, target_chain.0)
}

pub fn log_resumable(pair: Option<(ChainId, ChainId)>) {
    if let Some((src, dst)) = pair {
        info!(
            source_chain = src.0,
            target_chain = dst.0,
            "decommission: resumable drain detected"
        );
    } else {
        info!("decommission: no resumable drain detected");
    }
}
