//! Cold-path rebalancing module.
//!
//! Periodically monitors SyncUSD pool depths across all active chains,
//! detects imbalances against configurable per-chain thresholds, and
//! executes CCIP burn-and-mint operations to restore target ratios.
//!
//! # Assumed Bank Contract interface
//!
//! Write:
//!   `rebalance(uint64 destChainId, uint256 amount) payable`
//!   The contract burns `amount` of SyncUSD on the source chain, sends a CCIP
//!   message, and the CCIP receiver on the destination chain mints the same
//!   amount.  `msg.value` covers CCIP messaging fees.
//!
//! Read:
//!   `poolDepth() returns (uint256)`
//!
//! # Safety guarantees
//!
//! - Never rebalances more than `COLD_PATH_MAX_WEI` per operation.
//! - Verifies source pool has sufficient surplus before burning.
//! - Skips chains not in the latest `ActivationPublished` active set.
//! - Idempotency: skips chain pairs that already have a pending/submitted op
//!   created within the last 24 hours.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, Address, B256, U256};
use k256::ecdsa::SigningKey;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::domain::abi::{encode_rebalance, extract_ccip_message_id};
use crate::domain::rebalance::compute_rebalance_ops;
use crate::domain::repository::RebalanceRepository;
use crate::error::TxError;
use crate::eth;

// ── Constants ─────────────────────────────────────────────────────────────────

const ROUTE_RECEIVER_POLL_INTERVAL: Duration = Duration::from_secs(60);
const MAX_TX_WAIT: Duration = Duration::from_secs(120);
const MAX_RETRIES: u32 = 3;
const MAX_BLOCK_RANGE: u64 = 2_000;
/// Gas limit for a `rebalance()` call (includes CCIP sendMessage overhead).
const REBALANCE_GAS_LIMIT: u64 = 300_000;

// ── Cold-path module ──────────────────────────────────────────────────────────

pub struct ColdPath {
    rebalance_repo: Arc<dyn RebalanceRepository>,
    config: Arc<Config>,
    http: reqwest::Client,
    /// Active chain IDs from the latest `ActivationPublished` event.
    /// Seeded with all configured chains so rebalancing works before the
    /// first RouteReceiver publish arrives.
    active_chains: Arc<RwLock<HashSet<u64>>>,
    /// Relayer ECDSA signing key (absent when key file is missing/invalid).
    relayer_key: Option<Arc<SigningKey>>,
    /// Ethereum address derived from the relayer signing key.
    relayer_address: Option<Address>,
    /// Per-chain nonce cache to avoid redundant RPC round-trips.
    nonce_cache: Arc<Mutex<HashMap<u64, u64>>>,
    /// 4-byte selector for `poolDepth()`
    pool_depth_selector: [u8; 4],
    /// 4-byte selector for `rebalance(uint64,uint256)`
    rebalance_selector: [u8; 4],
    /// keccak256("ActivationPublished(string,string,string,uint256,string,string,uint256)")
    activation_topic: B256,
    /// Maximum SyncUSD (in wei) per rebalance operation. `None` = no cap.
    max_wei_per_op: Option<U256>,
    /// ETH (in wei) to include as `msg.value` for CCIP fees.
    ccip_fee_wei: U256,
    /// How often the monitor loop runs.
    poll_interval: Duration,
}

impl ColdPath {
    /// Construct a `ColdPath` and return it wrapped in an `Arc`.
    /// Call [`spawn_background`] on the result to start background tasks.
    pub fn new(rebalance_repo: Arc<dyn RebalanceRepository>, config: Arc<Config>, http: reqwest::Client) -> Arc<Self> {
        let (relayer_key, relayer_address) = eth::load_signing_key(&config.relayer_key_path);

        let initial: HashSet<u64> = config.rpc_urls.keys().cloned().collect();

        let pool_depth_hash = keccak256(b"poolDepth()");
        let mut pool_depth_selector = [0u8; 4];
        pool_depth_selector.copy_from_slice(&pool_depth_hash[..4]);

        let rebalance_hash = keccak256(b"rebalance(uint64,uint256)");
        let mut rebalance_selector = [0u8; 4];
        rebalance_selector.copy_from_slice(&rebalance_hash[..4]);

        let activation_topic =
            keccak256(b"ActivationPublished(string,string,string,uint256,string,string,uint256)");

        let max_wei_per_op = if config.cold_path_max_wei.is_empty() {
            None
        } else {
            match config.cold_path_max_wei.parse::<U256>() {
                Ok(v) => Some(v),
                Err(_) => {
                    warn!(
                        val = %config.cold_path_max_wei,
                        "cold_path: invalid COLD_PATH_MAX_WEI — treating as no cap"
                    );
                    None
                }
            }
        };

        let ccip_fee_wei = config.ccip_fee_wei.parse::<U256>().unwrap_or(U256::ZERO);

        let poll_interval = Duration::from_secs(config.cold_path_poll_secs);

        Arc::new(Self {
            rebalance_repo,
            config,
            http,
            active_chains: Arc::new(RwLock::new(initial)),
            relayer_key,
            relayer_address,
            nonce_cache: Arc::new(Mutex::new(HashMap::new())),
            pool_depth_selector,
            rebalance_selector,
            activation_topic,
            max_wei_per_op,
            ccip_fee_wei,
            poll_interval,
        })
    }

    /// Spawn the RouteReceiver monitor and the rebalance monitor loops.
    pub fn spawn_background(self: Arc<Self>) {
        let this = Arc::clone(&self);
        tokio::spawn(async move { this.poll_route_receiver_loop().await });
        tokio::spawn(async move { self.monitor_loop().await });
    }

    // ── Background loops ──────────────────────────────────────────────────────

    /// Main rebalancing loop: runs once per `poll_interval`.
    async fn monitor_loop(self: Arc<Self>) {
        info!(
            interval_secs = self.poll_interval.as_secs(),
            "cold_path: rebalance monitor started"
        );
        loop {
            self.run_rebalance_cycle().await;
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    /// Poll `RouteReceiver.sol` for `ActivationPublished` events and update
    /// the in-memory active-chain set.
    async fn poll_route_receiver_loop(self: Arc<Self>) {
        let rpc_url = match self.config.rpc_urls.values().next() {
            Some(u) => u.clone(),
            None => {
                warn!("cold_path: no RPC URLs — route receiver polling disabled");
                return;
            }
        };

        let mut last_block: u64 = 0;
        let topic = format!("{}", self.activation_topic);
        info!("cold_path: route receiver polling started");

        loop {
            let to_block = match eth::fetch_block_number(&self.http, &rpc_url).await {
                Some(b) => b,
                None => {
                    tokio::time::sleep(ROUTE_RECEIVER_POLL_INTERVAL).await;
                    continue;
                }
            };

            let scan_from = if last_block == 0 {
                to_block
            } else {
                last_block
            };
            let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

            let logs = eth::fetch_logs(
                &self.http,
                &rpc_url,
                &self.config.route_receiver_address,
                &topic,
                scan_from,
                to_block,
            )
            .await;

            for log in &logs {
                if let Some(chains) = eth::decode_active_chains_from_event(&log.data) {
                    info!(
                        chains = ?chains,
                        "cold_path: activation state updated from RouteReceiver"
                    );
                    *self.active_chains.write().await = chains;
                }
            }

            last_block = to_block + 1;
            tokio::time::sleep(ROUTE_RECEIVER_POLL_INTERVAL).await;
        }
    }

    // ── Core rebalancing logic ────────────────────────────────────────────────

    async fn run_rebalance_cycle(&self) {
        let active = self.active_chains.read().await.clone();

        // Collect chain info for active chains that have both an RPC and a contract address.
        let chains: Vec<(u64, String, String)> = self
            .config
            .rpc_urls
            .iter()
            .filter_map(|(&chain_id, rpc_url)| {
                if !active.contains(&chain_id) {
                    return None;
                }
                self.config
                    .contract_addresses
                    .get(&chain_id)
                    .map(|addr| (chain_id, rpc_url.clone(), addr.clone()))
            })
            .collect();

        if chains.len() < 2 {
            // Need at least two chains to move funds between.
            return;
        }

        // ── 1. Fetch current pool depths ──────────────────────────────────────
        let mut depths: HashMap<u64, U256> = HashMap::new();
        for (chain_id, rpc_url, bank_addr) in &chains {
            match eth::fetch_pool_depth(&self.http, rpc_url, bank_addr, &self.pool_depth_selector)
                .await
            {
                Some(d) => {
                    depths.insert(*chain_id, d);
                }
                None => {
                    warn!(
                        chain = chain_id,
                        "cold_path: failed to fetch pool depth — skipping cycle"
                    );
                    return;
                }
            }
        }

        let total: U256 = depths.values().fold(U256::ZERO, |acc, &d| acc + d);
        if total.is_zero() {
            info!("cold_path: total pool is zero — nothing to rebalance");
            return;
        }

        let n = depths.len() as u64;
        let min_bps = self.config.cold_path_min_bps;
        let target_bps = self.config.cold_path_target_bps;

        // Compute the target depth for a chain.
        let target_depth = |_chain_id: u64| -> U256 {
            if target_bps == 0 {
                // Equal distribution.
                total / U256::from(n)
            } else {
                total * U256::from(target_bps) / U256::from(10_000u64)
            }
        };

        // ── 2. Detect imbalances ──────────────────────────────────────────────
        let any_deficit = depths.iter().any(|(&chain_id, &depth)| {
            // ratio_bps = depth / total * 10000
            let ratio_bps = depth * U256::from(10_000u64) / total;
            let below = ratio_bps < U256::from(min_bps);
            if below {
                info!(
                    chain = chain_id,
                    ratio_bps = ratio_bps.to_string(),
                    min_bps,
                    "cold_path: chain below minimum pool ratio"
                );
            }
            below
        });

        if !any_deficit {
            info!("cold_path: all pools within thresholds — no rebalancing needed");
            return;
        }

        // ── 3. Compute surplus and deficit per chain ───────────────────────────
        let mut surpluses: Vec<(u64, U256)> = Vec::new();
        let mut deficits: Vec<(u64, U256)> = Vec::new();

        for (&chain_id, &depth) in &depths {
            let target = target_depth(chain_id);
            if depth > target {
                surpluses.push((chain_id, depth - target));
            } else if depth < target {
                deficits.push((chain_id, target - depth));
            }
        }

        // ── 4. Match surplus→deficit and apply per-op cap ─────────────────────
        let ops = compute_rebalance_ops(&surpluses, &deficits, self.max_wei_per_op);
        if ops.is_empty() {
            info!("cold_path: no rebalance operations computed");
            return;
        }

        info!(op_count = ops.len(), "cold_path: executing rebalance cycle");

        // ── 5. Submit each operation (sequentially to keep nonces ordered) ─────
        for (source_chain, dest_chain, amount) in ops {
            // Safety: verify source pool still has sufficient surplus.
            let source_rpc = match self.config.rpc_urls.get(&source_chain) {
                Some(u) => u.clone(),
                None => continue,
            };
            let source_bank = match self.config.contract_addresses.get(&source_chain) {
                Some(a) => a.clone(),
                None => continue,
            };
            match eth::fetch_pool_depth(
                &self.http,
                &source_rpc,
                &source_bank,
                &self.pool_depth_selector,
            )
            .await
            {
                Some(live_depth) if live_depth >= amount => {} // sufficient — proceed
                Some(live_depth) => {
                    warn!(
                        source_chain,
                        dest_chain,
                        amount = %amount,
                        depth = %live_depth,
                        "cold_path: source pool shrank — skipping this op"
                    );
                    continue;
                }
                None => {
                    warn!(
                        source_chain,
                        "cold_path: could not re-verify source depth — skipping"
                    );
                    continue;
                }
            }

            // Idempotency: skip if a recent op for this chain pair is still in flight.
            if self.rebalance_repo.op_in_flight(source_chain, dest_chain).await {
                info!(
                    source_chain,
                    dest_chain, "cold_path: in-flight op exists — skipping"
                );
                continue;
            }

            // Generate a unique op_id from chain IDs and wall-clock milliseconds.
            let ts_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let op_id = format!("{source_chain}-{dest_chain}-{ts_ms}");

            self.rebalance_repo.insert_rebalance_op(&op_id, source_chain, dest_chain, &amount)
                .await;

            match self
                .submit_rebalance_with_retry(
                    &op_id,
                    source_chain,
                    &source_rpc,
                    &source_bank,
                    dest_chain,
                    &amount,
                )
                .await
            {
                Ok((tx_hash, ccip_msg_id)) => {
                    info!(
                        op_id,
                        source_chain,
                        dest_chain,
                        amount = %amount,
                        tx_hash,
                        ccip_message_id = ccip_msg_id.as_deref().unwrap_or("unknown"),
                        "cold_path: rebalance submitted"
                    );
                    self.rebalance_repo.update_rebalance_op_submitted(&op_id, &tx_hash, ccip_msg_id.as_deref())
                        .await;
                }
                Err(e) => {
                    error!(op_id, err = %e, "cold_path: rebalance failed after retries");
                    self.rebalance_repo.update_rebalance_op_failed(&op_id).await;
                }
            }
        }
    }

    // ── Transaction submission ────────────────────────────────────────────────

    async fn submit_rebalance_with_retry(
        &self,
        op_id: &str,
        source_chain: u64,
        rpc_url: &str,
        bank_addr: &str,
        dest_chain: u64,
        amount: &U256,
    ) -> Result<(String, Option<String>), TxError> {
        let key = self
            .relayer_key
            .as_ref()
            .ok_or(TxError::MissingKey)?;

        let mut delay = Duration::from_secs(1);

        for attempt in 1..=MAX_RETRIES {
            match self
                .submit_rebalance_once(source_chain, rpc_url, bank_addr, dest_chain, amount, key)
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!(op_id, attempt, err = %e, "cold_path: rebalance attempt failed");
                    self.nonce_cache.lock().await.remove(&source_chain);
                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                    }
                }
            }
        }

        Err(TxError::RetryExhausted { attempts: MAX_RETRIES })
    }

    async fn submit_rebalance_once(
        &self,
        chain_id: u64,
        rpc_url: &str,
        bank_addr: &str,
        dest_chain_id: u64,
        amount: &U256,
        key: &SigningKey,
    ) -> Result<(String, Option<String>), TxError> {
        let relayer_addr = self
            .relayer_address
            .ok_or(TxError::MissingKey)?;

        let nonce = self.get_nonce(rpc_url, chain_id, &relayer_addr).await?;
        let (max_fee, max_priority_fee) = eth::fetch_gas_params(&self.http, rpc_url).await?;

        let call_data = encode_rebalance(&self.rebalance_selector, dest_chain_id, amount);

        let bank_addr_bytes = eth::decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or(TxError::InvalidAddress)?;

        let raw_hex = eth::sign_eip1559_tx(
            chain_id,
            nonce,
            max_priority_fee,
            max_fee,
            REBALANCE_GAS_LIMIT,
            &bank_addr_bytes,
            &eth::u256_to_trimmed_be(self.ccip_fee_wei),
            &call_data,
            key,
        )?;

        // Advance cached nonce before sending.
        self.nonce_cache.lock().await.insert(chain_id, nonce + 1);

        let tx_hash = eth::send_raw_transaction(&self.http, rpc_url, &raw_hex).await?;
        let receipt_logs =
            eth::wait_for_receipt_logs(&self.http, rpc_url, &tx_hash, MAX_TX_WAIT).await?;

        // Best-effort: extract CCIP messageId from tx logs (bytes32 in topics[1]
        // of any log that has at least 2 topics and a 32-byte first data topic).
        let ccip_message_id = extract_ccip_message_id(&receipt_logs);

        Ok((tx_hash, ccip_message_id))
    }

    // ── Nonce helper ─────────────────────────────────────────────────────────

    /// Get the nonce for a chain, checking the local cache first, then falling
    /// back to an RPC call via `eth::fetch_nonce`.
    async fn get_nonce(
        &self,
        rpc_url: &str,
        chain_id: u64,
        addr: &Address,
    ) -> Result<u64, TxError> {
        {
            let cache = self.nonce_cache.lock().await;
            if let Some(&n) = cache.get(&chain_id) {
                return Ok(n);
            }
        }
        eth::fetch_nonce(&self.http, rpc_url, addr).await
    }

}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::abi::encode_rebalance;

    #[test]
    fn encode_rebalance_calldata() {
        let sel = [0xde, 0xad, 0xbe, 0xef];
        let dest = 84532u64;
        let amount = U256::from(1_000_000_000_000_000_000u64); // 1e18
        let data = encode_rebalance(&sel, dest, &amount);
        assert_eq!(data.len(), 68);
        assert_eq!(&data[..4], &sel);
        // dest_chain_id sits in bytes 28..36 (last 8 of the 32-byte slot).
        let enc_dest = u64::from_be_bytes(data[28..36].try_into().unwrap());
        assert_eq!(enc_dest, dest);
        // amount is in bytes 36..68.
        let enc_amount = U256::from_be_bytes::<32>(data[36..68].try_into().unwrap());
        assert_eq!(enc_amount, amount);
    }
}
