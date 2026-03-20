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
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, Signature, SigningKey};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

use crate::config::Config;

// ── Constants ─────────────────────────────────────────────────────────────────

const ROUTE_RECEIVER_POLL_INTERVAL: Duration = Duration::from_secs(60);
const TX_RECEIPT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_TX_WAIT: Duration = Duration::from_secs(120);
const MAX_RETRIES: u32 = 3;
const MAX_BLOCK_RANGE: u64 = 2_000;
/// Gas limit for a `rebalance()` call (includes CCIP sendMessage overhead).
const REBALANCE_GAS_LIMIT: u64 = 300_000;

// ── JSON-RPC log ──────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct RpcLog {
    data: String,
}

// ── Cold-path module ──────────────────────────────────────────────────────────

pub struct ColdPath {
    pool: PgPool,
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
    pub fn new(pool: PgPool, config: Arc<Config>, http: reqwest::Client) -> Arc<Self> {
        let (relayer_key, relayer_address) = load_relayer_key(&config.relayer_key_path);

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
            pool,
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
            let to_block = match self.fetch_block_number(&rpc_url).await {
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

            let logs = self
                .fetch_logs(
                    &rpc_url,
                    &self.config.route_receiver_address,
                    &topic,
                    scan_from,
                    to_block,
                )
                .await;

            for log in &logs {
                if let Some(chains) = decode_active_chains_from_event(&log.data) {
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
            match self.fetch_pool_depth(rpc_url, bank_addr).await {
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
            match self.fetch_pool_depth(&source_rpc, &source_bank).await {
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
            if self.op_in_flight(source_chain, dest_chain).await {
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

            self.insert_rebalance_op(&op_id, source_chain, dest_chain, &amount)
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
                    self.update_rebalance_op_submitted(&op_id, &tx_hash, ccip_msg_id.as_deref())
                        .await;
                }
                Err(e) => {
                    error!(op_id, err = %e, "cold_path: rebalance failed after retries");
                    self.update_rebalance_op_failed(&op_id).await;
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
    ) -> Result<(String, Option<String>), String> {
        let key = self
            .relayer_key
            .as_ref()
            .ok_or_else(|| "relayer key not loaded".to_string())?;

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

        Err(format!("rebalance failed after {MAX_RETRIES} attempts"))
    }

    async fn submit_rebalance_once(
        &self,
        chain_id: u64,
        rpc_url: &str,
        bank_addr: &str,
        dest_chain_id: u64,
        amount: &U256,
        key: &SigningKey,
    ) -> Result<(String, Option<String>), String> {
        let relayer_addr = self
            .relayer_address
            .ok_or_else(|| "relayer address not set".to_string())?;

        let nonce = self.next_nonce(rpc_url, chain_id, &relayer_addr).await?;
        let (max_fee, max_priority_fee) = self.fetch_gas_params(rpc_url).await?;

        let call_data = encode_rebalance(&self.rebalance_selector, dest_chain_id, amount);

        let bank_addr_bytes = decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or("invalid bank contract address")?;

        // EIP-1559 signing payload: 0x02 || rlp([chain_id, nonce, ...])
        let signing_rlp = build_tx_rlp(
            chain_id,
            nonce,
            max_priority_fee,
            max_fee,
            REBALANCE_GAS_LIMIT,
            &bank_addr_bytes,
            &call_data,
            &self.ccip_fee_wei,
        );
        let mut to_sign = vec![0x02u8];
        to_sign.extend_from_slice(&signing_rlp);
        let hash = keccak256(&to_sign);

        let (sig, recid): (Signature, RecoveryId) = key
            .sign_prehash(hash.as_slice())
            .map_err(|e| e.to_string())?;

        let r_bytes = sig.r().to_bytes();
        let s_bytes = sig.s().to_bytes();
        let v = recid.to_byte() as u64;

        // Final signed transaction: 0x02 || rlp([..., v, r, s])
        let fee_bytes = u256_to_trimmed_be(self.ccip_fee_wei);
        let signed_rlp = rlp_encode_list(&[
            rlp_encode_uint(chain_id),
            rlp_encode_uint(nonce),
            rlp_encode_uint(max_priority_fee),
            rlp_encode_uint(max_fee),
            rlp_encode_uint(REBALANCE_GAS_LIMIT),
            rlp_encode_bytes(&bank_addr_bytes),
            rlp_encode_bytes(&fee_bytes), // msg.value = CCIP fee
            rlp_encode_bytes(&call_data),
            rlp_encode_list(&[]), // empty access list
            rlp_encode_uint(v),
            rlp_encode_bytes(&r_bytes),
            rlp_encode_bytes(&s_bytes),
        ]);
        let mut raw_tx = vec![0x02u8];
        raw_tx.extend_from_slice(&signed_rlp);
        let raw_hex = format!("0x{}", bytes_to_hex_raw(&raw_tx));

        // Advance cached nonce before sending.
        self.nonce_cache.lock().await.insert(chain_id, nonce + 1);

        let tx_hash = self.send_raw_transaction(rpc_url, &raw_hex).await?;
        let receipt_logs = self.wait_for_receipt_logs(rpc_url, &tx_hash).await?;

        // Best-effort: extract CCIP messageId from tx logs (bytes32 in topics[1]
        // of any log that has at least 2 topics and a 32-byte first data topic).
        let ccip_message_id = extract_ccip_message_id(&receipt_logs);

        Ok((tx_hash, ccip_message_id))
    }

    // ── RPC helpers ───────────────────────────────────────────────────────────

    async fn fetch_block_number(&self, rpc_url: &str) -> Option<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "method": "eth_blockNumber", "params": [], "id": 1
        });
        let resp: serde_json::Value = self
            .http
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let hex = resp["result"].as_str()?;
        u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
    }

    async fn fetch_logs(
        &self,
        rpc_url: &str,
        address: &str,
        topic: &str,
        from_block: u64,
        to_block: u64,
    ) -> Vec<RpcLog> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getLogs",
            "params": [{
                "address": address,
                "topics": [topic],
                "fromBlock": format!("0x{:x}", from_block),
                "toBlock":   format!("0x{:x}", to_block)
            }],
            "id": 1
        });
        let resp: serde_json::Value = match self.http.post(rpc_url).json(&body).send().await {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(_) => return vec![],
            },
            Err(_) => return vec![],
        };
        serde_json::from_value(resp["result"].clone()).unwrap_or_default()
    }

    async fn fetch_pool_depth(&self, rpc_url: &str, bank_addr: &str) -> Option<U256> {
        let call_data = format!("0x{}", bytes_to_hex_raw(&self.pool_depth_selector));
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{"to": bank_addr, "data": call_data}, "latest"],
            "id": 1
        });
        let resp: serde_json::Value = self
            .http
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let bytes = decode_hex(resp["result"].as_str()?)?;
        if bytes.len() < 32 {
            return None;
        }
        let arr: [u8; 32] = bytes[..32].try_into().ok()?;
        Some(U256::from_be_bytes(arr))
    }

    async fn next_nonce(
        &self,
        rpc_url: &str,
        chain_id: u64,
        addr: &Address,
    ) -> Result<u64, String> {
        {
            let cache = self.nonce_cache.lock().await;
            if let Some(&n) = cache.get(&chain_id) {
                return Ok(n);
            }
        }
        let addr_hex = format!("0x{}", bytes_to_hex_raw(addr.as_slice()));
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionCount",
            "params": [addr_hex, "pending"],
            "id": 1
        });
        let resp: serde_json::Value = self
            .http
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let hex = resp["result"].as_str().ok_or("missing nonce")?;
        u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(|e| e.to_string())
    }

    async fn fetch_gas_params(&self, rpc_url: &str) -> Result<(u64, u64), String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "method": "eth_gasPrice", "params": [], "id": 1
        });
        let resp: serde_json::Value = self
            .http
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let hex = resp["result"].as_str().ok_or("missing gasPrice")?;
        let gp =
            u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
        let tip = gp / 10;
        Ok((gp + tip, tip))
    }

    async fn send_raw_transaction(&self, rpc_url: &str, raw_hex: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendRawTransaction",
            "params": [raw_hex],
            "id": 1
        });
        let resp: serde_json::Value = self
            .http
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(err) = resp.get("error") {
            return Err(format!("eth_sendRawTransaction: {err}"));
        }
        resp["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no tx hash in response".to_string())
    }

    /// Wait for a transaction receipt and return the raw log entries.
    async fn wait_for_receipt_logs(
        &self,
        rpc_url: &str,
        tx_hash: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        let deadline = tokio::time::Instant::now() + MAX_TX_WAIT;
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1
        });
        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(format!("timed out waiting for receipt: {tx_hash}"));
            }
            let resp: serde_json::Value = self
                .http
                .post(rpc_url)
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?;
            if let Some(receipt) = resp["result"].as_object() {
                match receipt.get("status").and_then(|s| s.as_str()) {
                    Some("0x1") => {
                        let logs = receipt
                            .get("logs")
                            .and_then(|l| l.as_array())
                            .cloned()
                            .unwrap_or_default();
                        return Ok(logs);
                    }
                    Some("0x0") => return Err(format!("transaction reverted: {tx_hash}")),
                    _ => {}
                }
            }
            tokio::time::sleep(TX_RECEIPT_POLL_INTERVAL).await;
        }
    }

    // ── Database helpers ──────────────────────────────────────────────────────

    /// Returns true if there is a pending or submitted rebalance op for this
    /// chain pair created within the last 24 hours.
    async fn op_in_flight(&self, source_chain: u64, dest_chain: u64) -> bool {
        sqlx::query(
            "SELECT 1 FROM treasury.rebalance_ops \
             WHERE source_chain_id = $1 \
               AND dest_chain_id   = $2 \
               AND status IN ('pending', 'submitted') \
               AND created_at > NOW() - INTERVAL '24 hours'",
        )
        .bind(source_chain as i64)
        .bind(dest_chain as i64)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
    }

    async fn insert_rebalance_op(
        &self,
        op_id: &str,
        source_chain: u64,
        dest_chain: u64,
        amount: &U256,
    ) {
        let amount_str = amount.to_string();
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO treasury.rebalance_ops
                (op_id, source_chain_id, dest_chain_id, amount_wei, status)
            VALUES ($1, $2, $3, $4::NUMERIC, 'pending')
            ON CONFLICT (op_id) DO NOTHING
            "#,
        )
        .bind(op_id)
        .bind(source_chain as i64)
        .bind(dest_chain as i64)
        .bind(&amount_str)
        .execute(&self.pool)
        .await
        {
            error!(op_id, err = %e, "cold_path: failed to insert rebalance_op");
        }
    }

    async fn update_rebalance_op_submitted(
        &self,
        op_id: &str,
        tx_hash: &str,
        ccip_message_id: Option<&str>,
    ) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.rebalance_ops \
             SET status = 'submitted', source_tx_hash = $1, \
                 ccip_message_id = $2, updated_at = NOW() \
             WHERE op_id = $3",
        )
        .bind(tx_hash)
        .bind(ccip_message_id)
        .bind(op_id)
        .execute(&self.pool)
        .await
        {
            error!(op_id, err = %e, "cold_path: failed to update rebalance_op to submitted");
        }
    }

    async fn update_rebalance_op_failed(&self, op_id: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.rebalance_ops \
             SET status = 'failed', updated_at = NOW() \
             WHERE op_id = $1",
        )
        .bind(op_id)
        .execute(&self.pool)
        .await
        {
            error!(op_id, err = %e, "cold_path: failed to mark rebalance_op as failed");
        }
    }
}

// ── Rebalancing algorithm ─────────────────────────────────────────────────────

/// Greedily match surplus chains to deficit chains, emitting
/// `(source_chain_id, dest_chain_id, amount)` triples.
///
/// Both inputs are sorted by amount descending so the largest imbalances are
/// resolved first.  Each triple is capped at `max_per_op` when set.
fn compute_rebalance_ops(
    surpluses: &[(u64, U256)],
    deficits: &[(u64, U256)],
    max_per_op: Option<U256>,
) -> Vec<(u64, u64, U256)> {
    let mut ops = Vec::new();

    // Sort descending so we drain the biggest imbalances first.
    let mut sur: Vec<(u64, U256)> = surpluses.to_vec();
    let mut def: Vec<(u64, U256)> = deficits.to_vec();
    sur.sort_by(|a, b| b.1.cmp(&a.1));
    def.sort_by(|a, b| b.1.cmp(&a.1));

    // Mutable remaining amounts.
    let mut sur_rem: Vec<U256> = sur.iter().map(|(_, a)| *a).collect();
    let mut def_rem: Vec<U256> = def.iter().map(|(_, a)| *a).collect();

    let mut si = 0;
    let mut di = 0;

    while si < sur.len() && di < def.len() {
        if sur_rem[si].is_zero() {
            si += 1;
            continue;
        }
        if def_rem[di].is_zero() {
            di += 1;
            continue;
        }

        let mut amount = sur_rem[si].min(def_rem[di]);
        if let Some(cap) = max_per_op {
            amount = amount.min(cap);
        }

        if !amount.is_zero() {
            ops.push((sur[si].0, def[di].0, amount));
        }

        sur_rem[si] = sur_rem[si].saturating_sub(amount);
        def_rem[di] = def_rem[di].saturating_sub(amount);

        // If the cap split the allocation, advance neither pointer — next
        // iteration will emit another op for the same pair.
        if sur_rem[si].is_zero() {
            si += 1;
        }
        if def_rem[di].is_zero() {
            di += 1;
        }
    }

    ops
}

// ── CCIP message ID extraction ────────────────────────────────────────────────

/// Best-effort extraction of a CCIP `messageId` (bytes32) from the receipt
/// logs.  CCIP's `CCIPSendRequested` event has the messageId as the second
/// topic.  We look for the first log that has exactly 2 topics where the
/// second is a valid 32-byte hex value.
fn extract_ccip_message_id(logs: &[serde_json::Value]) -> Option<String> {
    for log in logs {
        if let Some(topics) = log["topics"].as_array() {
            if topics.len() >= 2 {
                if let Some(topic1) = topics[1].as_str() {
                    // topics[1] is 32-byte hex (0x-prefixed, 66 chars).
                    if topic1.len() == 66 {
                        return Some(topic1.to_string());
                    }
                }
            }
        }
    }
    None
}

// ── Standalone helpers ────────────────────────────────────────────────────────

/// Load the relayer signing key from a hex-encoded file and derive its address.
fn load_relayer_key(path: &str) -> (Option<Arc<SigningKey>>, Option<Address>) {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let hex = contents.trim().trim_start_matches("0x");
    let bytes = match decode_hex(hex) {
        Some(b) if b.len() == 32 => b,
        _ => return (None, None),
    };
    let key = match SigningKey::from_bytes(bytes.as_slice().into()) {
        Ok(k) => k,
        Err(_) => return (None, None),
    };
    let uncompressed = key.verifying_key().to_encoded_point(false);
    let hash = keccak256(&uncompressed.as_bytes()[1..]); // skip 0x04 prefix
    let addr = Address::from_slice(&hash[12..]);
    (Some(Arc::new(key)), Some(addr))
}

/// Decode a hex string (with or without `0x` prefix) into bytes.
fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Encode a byte slice as lowercase hex (no `0x` prefix).
fn bytes_to_hex_raw(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

/// ABI-encode `rebalance(uint64 destChainId, uint256 amount)` call data.
///
/// ABI head layout:
///   4 bytes  selector
///  32 bytes  destChainId  (uint64, right-aligned)
///  32 bytes  amount       (uint256, big-endian)
fn encode_rebalance(selector: &[u8; 4], dest_chain_id: u64, amount: &U256) -> Vec<u8> {
    let mut data = Vec::with_capacity(68);
    data.extend_from_slice(selector);
    // uint64 → 32-byte slot (zero-padded, right-aligned)
    data.extend_from_slice(&[0u8; 24]);
    data.extend_from_slice(&dest_chain_id.to_be_bytes());
    // uint256 → 32-byte slot (big-endian)
    data.extend_from_slice(&amount.to_be_bytes::<32>());
    data
}

/// Decode the `activeChainsCsv` from an `ActivationPublished` event data field.
/// (Identical logic to hot_path — kept local so modules stay self-contained.)
fn decode_active_chains_from_event(hex_data: &str) -> Option<HashSet<u64>> {
    let data = decode_hex(hex_data)?;
    if data.len() < 6 * 32 {
        return None;
    }
    let ptr = u64::from_be_bytes(data[3 * 32 + 24..4 * 32].try_into().ok()?) as usize;
    if ptr + 32 > data.len() {
        return None;
    }
    let len = u64::from_be_bytes(data[ptr + 24..ptr + 32].try_into().ok()?) as usize;
    if ptr + 32 + len > data.len() {
        return None;
    }
    let csv = std::str::from_utf8(&data[ptr + 32..ptr + 32 + len]).ok()?;
    Some(
        csv.split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect(),
    )
}

/// Convert a U256 to its minimal big-endian byte representation (no leading zeros).
/// Returns `[0x00]` for zero to satisfy RLP encoding rules for the value field.
fn u256_to_trimmed_be(val: U256) -> Vec<u8> {
    if val.is_zero() {
        return vec![]; // RLP value 0 is encoded as empty bytes (0x80)
    }
    let bytes = val.to_be_bytes::<32>();
    let start = bytes.iter().position(|&x| x != 0).unwrap_or(31);
    bytes[start..].to_vec()
}

// ── Minimal RLP encoding ──────────────────────────────────────────────────────

fn rlp_encode_uint(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![0x80];
    }
    let b = val.to_be_bytes();
    let start = b.iter().position(|&x| x != 0).unwrap_or(7);
    rlp_encode_bytes(&b[start..])
}

fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    let mut out = Vec::new();
    if data.len() <= 55 {
        out.push(0x80 + data.len() as u8);
    } else {
        let lb = data.len().to_be_bytes();
        let ls = lb.iter().position(|&x| x != 0).unwrap_or(7);
        let lm = &lb[ls..];
        out.push(0xb7 + lm.len() as u8);
        out.extend_from_slice(lm);
    }
    out.extend_from_slice(data);
    out
}

fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = items.iter().flat_map(|i| i.iter().cloned()).collect();
    let mut out = Vec::new();
    if payload.len() <= 55 {
        out.push(0xc0 + payload.len() as u8);
    } else {
        let lb = payload.len().to_be_bytes();
        let ls = lb.iter().position(|&x| x != 0).unwrap_or(7);
        let lm = &lb[ls..];
        out.push(0xf7 + lm.len() as u8);
        out.extend_from_slice(lm);
    }
    out.extend_from_slice(&payload);
    out
}

/// Build the EIP-1559 RLP body used for both signing and the signed tx.
/// Includes `value` (for CCIP fees via `msg.value`).
fn build_tx_rlp(
    chain_id: u64,
    nonce: u64,
    max_priority_fee: u64,
    max_fee: u64,
    gas_limit: u64,
    to: &[u8],
    data: &[u8],
    value: &U256,
) -> Vec<u8> {
    let value_bytes = u256_to_trimmed_be(*value);
    rlp_encode_list(&[
        rlp_encode_uint(chain_id),
        rlp_encode_uint(nonce),
        rlp_encode_uint(max_priority_fee),
        rlp_encode_uint(max_fee),
        rlp_encode_uint(gas_limit),
        rlp_encode_bytes(to),
        rlp_encode_bytes(&value_bytes),
        rlp_encode_bytes(data),
        rlp_encode_list(&[]), // empty access list
    ])
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebalance_ops_basic() {
        // 3 chains, total = 1000:
        //   chain A = 600 (surplus 100 vs equal target 333)
        //   chain B = 300 (deficit 33)
        //   chain C = 100 (deficit 233)
        let surpluses = vec![(1u64, U256::from(267u64))];
        let deficits = vec![(3u64, U256::from(233u64)), (2u64, U256::from(34u64))];

        let ops = compute_rebalance_ops(&surpluses, &deficits, None);
        // Surplus is 267; fills 233 to chain 3, then 34 to chain 2.
        assert_eq!(ops.len(), 2);
        let total_moved: U256 = ops
            .iter()
            .map(|(_, _, a)| *a)
            .fold(U256::ZERO, |acc, a| acc + a);
        assert_eq!(total_moved, U256::from(267u64));
    }

    #[test]
    fn rebalance_ops_capped() {
        // Surplus of 500, deficit of 500, cap of 200 → 3 ops (200+200+100).
        let surpluses = vec![(1u64, U256::from(500u64))];
        let deficits = vec![(2u64, U256::from(500u64))];
        let cap = Some(U256::from(200u64));

        let ops = compute_rebalance_ops(&surpluses, &deficits, cap);
        assert_eq!(ops.len(), 3);
        let total_moved: U256 = ops
            .iter()
            .map(|(_, _, a)| *a)
            .fold(U256::ZERO, |acc, a| acc + a);
        assert_eq!(total_moved, U256::from(500u64));
        // Each individual op must be ≤ 200.
        for (_, _, amount) in &ops {
            assert!(*amount <= U256::from(200u64));
        }
    }

    #[test]
    fn rebalance_ops_no_deficit() {
        let surpluses = vec![(1u64, U256::from(100u64))];
        let deficits: Vec<(u64, U256)> = vec![];
        let ops = compute_rebalance_ops(&surpluses, &deficits, None);
        assert!(ops.is_empty());
    }

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

    #[test]
    fn u256_trimmed_be_zero() {
        assert!(u256_to_trimmed_be(U256::ZERO).is_empty());
    }

    #[test]
    fn u256_trimmed_be_nonzero() {
        let v = U256::from(0x1234u64);
        let b = u256_to_trimmed_be(v);
        assert_eq!(b, vec![0x12, 0x34]);
    }
}
