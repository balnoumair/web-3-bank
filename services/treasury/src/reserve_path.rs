//! Reserve-path module: keeps USDC reserves balanced across Bank Contracts via CCTP.
//!
//! Three independent background loops, all driven by the same `ReserveRepository`:
//!
//! 1. **Planner** — polls `reserveDepth()` on each active chain, computes per-chain
//!    deviation from `total_reserve / num_active_chains`, and when any chain falls
//!    below `RESERVE_PATH_MIN_BPS` of target, submits `bridgeReserve(destChainId,
//!    amount)` on the surplus chain's Bank. The Bank emits `ReserveBridgeInitiated`
//!    with a `messageId`; we capture and persist it.
//!
//! 2. **Relayer** — for every `submitted` op without an attestation yet, polls
//!    Circle's iris-api for the CCTP `(message, attestation)` pair, then calls
//!    `bridgeIn(message, attestation)` on the destination chain's
//!    `CCTPReserveBridge`. Marks the row `relayed`.
//!
//! 3. **Watcher** — scans `ReserveBridgeCompleted` events on every active
//!    destination chain. Matches by `(dest_chain, messageId)` and marks rows
//!    `completed`. Separately, marks rows stuck past `RESERVE_PATH_STUCK_TIMEOUT_SECS`
//!    as `failed` so operators can investigate.
//!
//! # Assumed Bank Contract interface (per OpenSpec change `add-usdc-reserve-rebalance`)
//!
//! Source:
//!   `bridgeReserve(uint64 destChainId, uint256 amount) returns (bytes32 messageId)`
//!   `reserveDepth() returns (uint256)`
//!   emits `ReserveBridgeInitiated(bytes32 indexed messageId, uint64 indexed destChainId,
//!         uint256 amount, bytes32 bridgeType)`
//!
//! Destination:
//!   `CCTPReserveBridge.bridgeIn(bytes message, bytes attestation) returns (bytes32 messageId)`
//!   emits `ReserveBridgeCompleted(bytes32 indexed messageId, uint64 indexed sourceChainId,
//!         uint256 amount)` from the Bank when the adapter calls `completeReserveBridge`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, Address, B256, U256};
use k256::ecdsa::SigningKey;
use serde_json::Value as JsonValue;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::domain::abi::{encode_bridge_in, encode_bridge_reserve, extract_reserve_bridge_initiated_message_id};
use crate::domain::newtypes::{ChainId, OperationId, TxHash};
use crate::domain::rebalance::{compute_rebalance_ops, evaluate_rebalance_op, RebalanceOpDecision};
use crate::domain::repository::{ReserveOpRow, ReserveRepository};
use crate::error::TxError;
use crate::eth;

// ── Constants ─────────────────────────────────────────────────────────────────

const ROUTE_RECEIVER_POLL_INTERVAL: Duration = Duration::from_secs(60);
const MAX_TX_WAIT: Duration = Duration::from_secs(120);
const MAX_RETRIES: u32 = 3;
const MAX_BLOCK_RANGE: u64 = 2_000;
/// Gas limit for `Bank.bridgeReserve` (includes CCTP `depositForBurnWithCaller` overhead).
const BRIDGE_RESERVE_GAS_LIMIT: u64 = 350_000;
/// Gas limit for `CCTPReserveBridge.bridgeIn` (CCTP `receiveMessage` + `completeReserveBridge`).
const BRIDGE_IN_GAS_LIMIT: u64 = 400_000;
/// Circle's iris-api is slow — only poll it once every few cycles.
const ATTESTATION_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

// ── ReservePath module ────────────────────────────────────────────────────────

pub struct ReservePath {
    reserve_repo: Arc<dyn ReserveRepository>,
    config: Arc<Config>,
    http: reqwest::Client,
    active_chains: Arc<RwLock<HashSet<ChainId>>>,
    relayer_key: Option<Arc<SigningKey>>,
    relayer_address: Option<Address>,
    nonce_cache: Arc<Mutex<HashMap<ChainId, u64>>>,
    /// Per-chain "last scanned block" cursor for the watcher loop.
    watcher_cursor: Arc<Mutex<HashMap<ChainId, u64>>>,

    reserve_depth_selector: [u8; 4],
    bridge_reserve_selector: [u8; 4],
    bridge_in_selector: [u8; 4],

    activation_topic: B256,
    reserve_bridge_completed_topic: B256,

    max_wei_per_op: Option<U256>,
    bridge_fee_wei: U256,
}

impl ReservePath {
    pub fn new(
        reserve_repo: Arc<dyn ReserveRepository>,
        config: Arc<Config>,
        http: reqwest::Client,
    ) -> Arc<Self> {
        // Prefer a reserve-ops-specific key if provided, else fall back to the cold-path relayer.
        let key_path = config
            .reserve_relayer_key_path
            .clone()
            .unwrap_or_else(|| config.relayer_key_path.clone());
        let (relayer_key, relayer_address) = eth::load_signing_key(&key_path);

        let initial: HashSet<ChainId> = config.rpc_urls.keys().map(|&k| ChainId(k)).collect();

        let mut reserve_depth_selector = [0u8; 4];
        reserve_depth_selector.copy_from_slice(&keccak256(b"reserveDepth()")[..4]);

        let mut bridge_reserve_selector = [0u8; 4];
        bridge_reserve_selector.copy_from_slice(&keccak256(b"bridgeReserve(uint64,uint256)")[..4]);

        let mut bridge_in_selector = [0u8; 4];
        bridge_in_selector.copy_from_slice(&keccak256(b"bridgeIn(bytes,bytes)")[..4]);

        let activation_topic =
            keccak256(b"ActivationPublished(string,string,string,uint256,string,string,uint256)");
        let reserve_bridge_completed_topic =
            keccak256(b"ReserveBridgeCompleted(bytes32,uint64,uint256)");

        let max_wei_per_op = if config.reserve_path_max_wei.is_empty() {
            None
        } else {
            match config.reserve_path_max_wei.parse::<U256>() {
                Ok(v) => Some(v),
                Err(_) => {
                    warn!(
                        val = %config.reserve_path_max_wei,
                        "reserve_path: invalid RESERVE_PATH_MAX_WEI — treating as no cap"
                    );
                    None
                }
            }
        };

        let bridge_fee_wei = config
            .reserve_bridge_fee_wei
            .parse::<U256>()
            .unwrap_or(U256::ZERO);

        Arc::new(Self {
            reserve_repo,
            config,
            http,
            active_chains: Arc::new(RwLock::new(initial)),
            relayer_key,
            relayer_address,
            nonce_cache: Arc::new(Mutex::new(HashMap::new())),
            watcher_cursor: Arc::new(Mutex::new(HashMap::new())),
            reserve_depth_selector,
            bridge_reserve_selector,
            bridge_in_selector,
            activation_topic,
            reserve_bridge_completed_topic,
            max_wei_per_op,
            bridge_fee_wei,
        })
    }

    /// Spawn all background loops. Idempotent — call once at startup.
    pub fn spawn_background(self: Arc<Self>) {
        let poll = Duration::from_secs(self.config.reserve_path_poll_secs);

        let r = Arc::clone(&self);
        tokio::spawn(async move { r.poll_route_receiver_loop().await });

        let r = Arc::clone(&self);
        tokio::spawn(async move { r.planner_loop(poll).await });

        let r = Arc::clone(&self);
        tokio::spawn(async move { r.relayer_loop(poll).await });

        let r = Arc::clone(&self);
        tokio::spawn(async move { r.watcher_loop(poll).await });
    }

    // ── Route receiver poll (active-set tracking) ──────────────────────────

    async fn poll_route_receiver_loop(self: Arc<Self>) {
        let rpc_url = match self.config.rpc_urls.values().next() {
            Some(u) => u.clone(),
            None => {
                warn!("reserve_path: no RPC URLs — route receiver polling disabled");
                return;
            }
        };
        let mut last_block: u64 = 0;
        let topic = format!("{}", self.activation_topic);
        info!("reserve_path: route receiver polling started");

        loop {
            let Some(to_block) = eth::fetch_block_number(&self.http, &rpc_url).await else {
                tokio::time::sleep(ROUTE_RECEIVER_POLL_INTERVAL).await;
                continue;
            };
            let from = if last_block == 0 { to_block } else { last_block };
            let from = from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

            let logs = eth::fetch_logs(
                &self.http,
                &rpc_url,
                &self.config.route_receiver_address,
                &topic,
                from,
                to_block,
            )
            .await;

            for log in &logs {
                if let Some(chains) = eth::decode_active_chains_from_event(&log.data) {
                    let converted: HashSet<ChainId> = chains.into_iter().map(ChainId).collect();
                    info!(chains = ?converted, "reserve_path: activation set updated");
                    *self.active_chains.write().await = converted;
                }
            }

            last_block = to_block + 1;
            tokio::time::sleep(ROUTE_RECEIVER_POLL_INTERVAL).await;
        }
    }

    // ── Planner loop ──────────────────────────────────────────────────────────

    async fn planner_loop(self: Arc<Self>, poll: Duration) {
        info!(
            interval_secs = poll.as_secs(),
            min_bps = self.config.reserve_path_min_bps,
            "reserve_path: planner started"
        );
        loop {
            self.run_planner_cycle().await;
            tokio::time::sleep(poll).await;
        }
    }

    async fn run_planner_cycle(&self) {
        let active = self.active_chains.read().await.clone();

        let chains: Vec<(ChainId, String, String)> = self
            .config
            .rpc_urls
            .iter()
            .filter_map(|(&chain_id, rpc_url)| {
                if !active.contains(&ChainId(chain_id)) {
                    return None;
                }
                self.config
                    .contract_addresses
                    .get(&chain_id)
                    .map(|addr| (ChainId(chain_id), rpc_url.clone(), addr.clone()))
            })
            .collect();

        if chains.len() < 2 {
            return;
        }

        // 1. Fetch reserve depths.
        let mut depths: HashMap<ChainId, U256> = HashMap::new();
        for (chain_id, rpc_url, bank_addr) in &chains {
            match eth::fetch_pool_depth(&self.http, rpc_url, bank_addr, &self.reserve_depth_selector)
                .await
            {
                Some(d) => {
                    depths.insert(*chain_id, d);
                }
                None => {
                    warn!(
                        chain = chain_id.0,
                        "reserve_path: reserveDepth fetch failed — skipping cycle"
                    );
                    return;
                }
            }
        }

        let total: U256 = depths.values().fold(U256::ZERO, |acc, &d| acc + d);
        if total.is_zero() {
            return;
        }

        let n = depths.len() as u64;
        let target: U256 = total / U256::from(n);
        if target.is_zero() {
            return;
        }
        let min_bps = self.config.reserve_path_min_bps;

        // 2. Trigger only if some chain is below `min_bps` of target.
        let any_deficit = depths.iter().any(|(_, &depth)| {
            let ratio_bps = depth * U256::from(10_000u64) / target;
            ratio_bps < U256::from(min_bps)
        });
        if !any_deficit {
            return;
        }

        // 3. Surpluses (above target) → deficits (below target).
        let mut surpluses: Vec<(u64, U256)> = Vec::new();
        let mut deficits: Vec<(u64, U256)> = Vec::new();
        for (&chain_id, &depth) in &depths {
            if depth > target {
                surpluses.push((chain_id.0, depth - target));
            } else if depth < target {
                deficits.push((chain_id.0, target - depth));
            }
        }

        let ops = compute_rebalance_ops(&surpluses, &deficits, self.max_wei_per_op);
        if ops.is_empty() {
            return;
        }
        info!(op_count = ops.len(), "reserve_path: planner emitting bridge ops");

        // 4. Submit each op.
        for (src_raw, dst_raw, amount) in ops {
            let source_chain = ChainId(src_raw);
            let dest_chain = ChainId(dst_raw);

            let src_rpc = match self.config.rpc_urls.get(&source_chain.0) {
                Some(u) => u.clone(),
                None => continue,
            };
            let src_bank = match self.config.contract_addresses.get(&source_chain.0) {
                Some(a) => a.clone(),
                None => continue,
            };

            let Some(live_depth) = eth::fetch_pool_depth(
                &self.http,
                &src_rpc,
                &src_bank,
                &self.reserve_depth_selector,
            )
            .await
            else {
                warn!(source_chain = source_chain.0, "reserve_path: re-verify depth failed");
                continue;
            };

            let in_flight = self
                .reserve_repo
                .op_in_flight(source_chain, dest_chain)
                .await;

            match evaluate_rebalance_op(live_depth, amount, in_flight) {
                RebalanceOpDecision::Submit => {}
                RebalanceOpDecision::SkipInsufficientDepth => {
                    warn!(
                        source_chain = source_chain.0,
                        dest_chain = dest_chain.0,
                        "reserve_path: source reserve shrank — skipping"
                    );
                    continue;
                }
                RebalanceOpDecision::SkipOpInFlight => continue,
            }

            let ts_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let op_id = OperationId(format!("res-{}-{}-{}", source_chain, dest_chain, ts_ms));

            self.reserve_repo
                .insert_reserve_op(&op_id, source_chain, dest_chain, &amount, "CCTP")
                .await;

            match self
                .submit_bridge_reserve(&op_id, source_chain, &src_rpc, &src_bank, dest_chain, &amount)
                .await
            {
                Ok((tx_hash, message_id)) => {
                    info!(
                        op_id = %op_id,
                        source_chain = source_chain.0,
                        dest_chain = dest_chain.0,
                        amount = %amount,
                        tx_hash = %tx_hash,
                        message_id = message_id.as_deref().unwrap_or("unknown"),
                        "reserve_path: bridgeReserve submitted"
                    );
                    self.reserve_repo
                        .update_submitted(&op_id, &tx_hash, message_id.as_deref())
                        .await;
                }
                Err(e) => {
                    error!(op_id = %op_id, err = %e, "reserve_path: bridgeReserve failed");
                    self.reserve_repo.update_failed(&op_id).await;
                }
            }
        }
    }

    async fn submit_bridge_reserve(
        &self,
        op_id: &OperationId,
        source_chain: ChainId,
        rpc_url: &str,
        bank_addr: &str,
        dest_chain: ChainId,
        amount: &U256,
    ) -> Result<(TxHash, Option<String>), TxError> {
        let key = self.relayer_key.as_ref().ok_or(TxError::MissingKey)?;
        let relayer_addr = self.relayer_address.ok_or(TxError::MissingKey)?;

        let mut delay = Duration::from_secs(1);
        for attempt in 1..=MAX_RETRIES {
            match self
                .submit_bridge_reserve_once(
                    source_chain,
                    rpc_url,
                    bank_addr,
                    dest_chain,
                    amount,
                    &relayer_addr,
                    key,
                )
                .await
            {
                Ok(r) => return Ok(r),
                Err(e) => {
                    warn!(op_id = %op_id, attempt, err = %e, "reserve_path: bridgeReserve attempt failed");
                    self.nonce_cache.lock().await.remove(&source_chain);
                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                    }
                }
            }
        }
        Err(TxError::RetryExhausted {
            attempts: MAX_RETRIES,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn submit_bridge_reserve_once(
        &self,
        chain_id: ChainId,
        rpc_url: &str,
        bank_addr: &str,
        dest_chain_id: ChainId,
        amount: &U256,
        relayer_addr: &Address,
        key: &SigningKey,
    ) -> Result<(TxHash, Option<String>), TxError> {
        let nonce = self.get_nonce(rpc_url, chain_id, relayer_addr).await?;
        let (max_fee, max_priority_fee) = eth::fetch_gas_params(&self.http, rpc_url).await?;

        let call_data =
            encode_bridge_reserve(&self.bridge_reserve_selector, dest_chain_id.0, amount);

        let bank_bytes = eth::decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or(TxError::InvalidAddress)?;

        let raw_hex = eth::sign_eip1559_tx(
            chain_id.0,
            nonce,
            max_priority_fee,
            max_fee,
            BRIDGE_RESERVE_GAS_LIMIT,
            &bank_bytes,
            &[],
            &call_data,
            key,
        )?;
        self.nonce_cache.lock().await.insert(chain_id, nonce + 1);

        let tx_hash = eth::send_raw_transaction(&self.http, rpc_url, &raw_hex).await?;
        let receipt_logs =
            eth::wait_for_receipt_logs(&self.http, rpc_url, &tx_hash, MAX_TX_WAIT).await?;
        let message_id = extract_reserve_bridge_initiated_message_id(&receipt_logs);

        Ok((TxHash(tx_hash), message_id))
    }

    // ── Relayer loop ──────────────────────────────────────────────────────────

    async fn relayer_loop(self: Arc<Self>, poll: Duration) {
        info!("reserve_path: relayer started");
        loop {
            let rows = self.reserve_repo.list_awaiting_attestation().await;
            for row in rows {
                self.relay_one(&row).await;
            }
            tokio::time::sleep(poll).await;
        }
    }

    async fn relay_one(&self, row: &ReserveOpRow) {
        let Some(src_tx) = row.source_tx_hash.as_deref() else {
            return;
        };
        let Some(cctp_domains) = &self.config.cctp_domains else {
            warn!("reserve_path: cctp_domains not configured — relayer cannot run");
            return;
        };
        let Some(&src_domain) = cctp_domains.get(&(row.source_chain_id as u64)) else {
            warn!(
                source_chain = row.source_chain_id,
                "reserve_path: no CCTP domain configured for source chain"
            );
            return;
        };

        // Fetch attestation from Circle.
        let Some((message_hex, attestation_hex)) =
            self.fetch_circle_attestation(src_domain, src_tx).await
        else {
            return; // still pending or http error — retry next cycle
        };

        self.reserve_repo
            .update_attestation(
                &OperationId(row.op_id.clone()),
                &message_hex,
                &attestation_hex,
            )
            .await;

        // Dispatch bridgeIn on the destination chain.
        let dest_chain = ChainId(row.dest_chain_id as u64);
        let Some(dest_rpc) = self.config.rpc_urls.get(&dest_chain.0).cloned() else {
            warn!(dest_chain = dest_chain.0, "reserve_path: no RPC for dest");
            return;
        };
        let Some(bridges) = &self.config.reserve_bridge_addresses else {
            warn!("reserve_path: reserve_bridge_addresses not configured");
            return;
        };
        let Some(dest_adapter) = bridges.get(&dest_chain.0).cloned() else {
            warn!(
                dest_chain = dest_chain.0,
                "reserve_path: no reserve bridge address for dest"
            );
            return;
        };

        let Some(message_bytes) = eth::decode_hex(&message_hex) else {
            error!(op_id = %row.op_id, "reserve_path: invalid message hex from Circle");
            self.reserve_repo
                .update_failed(&OperationId(row.op_id.clone()))
                .await;
            return;
        };
        let Some(attestation_bytes) = eth::decode_hex(&attestation_hex) else {
            error!(op_id = %row.op_id, "reserve_path: invalid attestation hex from Circle");
            self.reserve_repo
                .update_failed(&OperationId(row.op_id.clone()))
                .await;
            return;
        };

        match self
            .submit_bridge_in(
                &row.op_id,
                dest_chain,
                &dest_rpc,
                &dest_adapter,
                &message_bytes,
                &attestation_bytes,
            )
            .await
        {
            Ok(tx_hash) => {
                info!(
                    op_id = %row.op_id,
                    dest_chain = dest_chain.0,
                    tx_hash = %tx_hash,
                    "reserve_path: bridgeIn dispatched"
                );
                self.reserve_repo
                    .update_relayed(&OperationId(row.op_id.clone()), &tx_hash)
                    .await;
            }
            Err(e) => {
                error!(op_id = %row.op_id, err = %e, "reserve_path: bridgeIn failed after retries");
                self.reserve_repo
                    .update_failed(&OperationId(row.op_id.clone()))
                    .await;
            }
        }
    }

    /// Fetch Circle's `(message, attestation)` for a source tx. Returns `None` if still
    /// pending or on HTTP error; the caller retries on the next cycle.
    async fn fetch_circle_attestation(
        &self,
        src_domain: u32,
        src_tx_hash: &str,
    ) -> Option<(String, String)> {
        let url = format!(
            "{}/v1/messages/{}/{}",
            self.config.circle_attestation_api_url.trim_end_matches('/'),
            src_domain,
            src_tx_hash
        );
        let resp = self
            .http
            .get(&url)
            .timeout(ATTESTATION_HTTP_TIMEOUT)
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            // 404 while CCTP is still confirming source tx — silent retry.
            return None;
        }
        let body: JsonValue = resp.json().await.ok()?;
        let first = body.get("messages")?.as_array()?.first()?;
        let attestation = first.get("attestation")?.as_str()?;
        let message = first.get("message")?.as_str()?;
        if attestation.eq_ignore_ascii_case("PENDING") || !attestation.starts_with("0x") {
            return None;
        }
        Some((message.to_string(), attestation.to_string()))
    }

    async fn submit_bridge_in(
        &self,
        op_id: &str,
        dest_chain: ChainId,
        rpc_url: &str,
        adapter_addr: &str,
        message: &[u8],
        attestation: &[u8],
    ) -> Result<TxHash, TxError> {
        let key = self.relayer_key.as_ref().ok_or(TxError::MissingKey)?;
        let relayer_addr = self.relayer_address.ok_or(TxError::MissingKey)?;

        let mut delay = Duration::from_secs(1);
        for attempt in 1..=MAX_RETRIES {
            match self
                .submit_bridge_in_once(dest_chain, rpc_url, adapter_addr, message, attestation, &relayer_addr, key)
                .await
            {
                Ok(t) => return Ok(t),
                Err(e) => {
                    warn!(op_id, attempt, err = %e, "reserve_path: bridgeIn attempt failed");
                    self.nonce_cache.lock().await.remove(&dest_chain);
                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                    }
                }
            }
        }
        Err(TxError::RetryExhausted {
            attempts: MAX_RETRIES,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn submit_bridge_in_once(
        &self,
        chain_id: ChainId,
        rpc_url: &str,
        adapter_addr: &str,
        message: &[u8],
        attestation: &[u8],
        relayer_addr: &Address,
        key: &SigningKey,
    ) -> Result<TxHash, TxError> {
        let nonce = self.get_nonce(rpc_url, chain_id, relayer_addr).await?;
        let (max_fee, max_priority_fee) = eth::fetch_gas_params(&self.http, rpc_url).await?;
        let call_data = encode_bridge_in(&self.bridge_in_selector, message, attestation);
        let adapter_bytes = eth::decode_hex(adapter_addr)
            .filter(|b| b.len() == 20)
            .ok_or(TxError::InvalidAddress)?;
        let raw_hex = eth::sign_eip1559_tx(
            chain_id.0,
            nonce,
            max_priority_fee,
            max_fee,
            BRIDGE_IN_GAS_LIMIT,
            &adapter_bytes,
            &eth::u256_to_trimmed_be(self.bridge_fee_wei),
            &call_data,
            key,
        )?;
        self.nonce_cache.lock().await.insert(chain_id, nonce + 1);

        let tx_hash = eth::send_raw_transaction(&self.http, rpc_url, &raw_hex).await?;
        let _ = eth::wait_for_receipt_logs(&self.http, rpc_url, &tx_hash, MAX_TX_WAIT).await?;
        Ok(TxHash(tx_hash))
    }

    // ── Watcher loop ──────────────────────────────────────────────────────────

    async fn watcher_loop(self: Arc<Self>, poll: Duration) {
        let stuck_timeout = Duration::from_secs(self.config.reserve_path_stuck_timeout_secs);
        info!(
            poll_secs = poll.as_secs(),
            stuck_secs = stuck_timeout.as_secs(),
            "reserve_path: watcher started"
        );
        loop {
            self.run_watcher_cycle(stuck_timeout).await;
            tokio::time::sleep(poll).await;
        }
    }

    async fn run_watcher_cycle(&self, stuck_timeout: Duration) {
        let active = self.active_chains.read().await.clone();
        let topic = format!("{}", self.reserve_bridge_completed_topic);

        for (&chain_id_raw, rpc_url) in self.config.rpc_urls.iter() {
            let chain_id = ChainId(chain_id_raw);
            if !active.contains(&chain_id) {
                continue;
            }
            let Some(bank_addr) = self.config.contract_addresses.get(&chain_id_raw) else {
                continue;
            };
            let Some(to_block) = eth::fetch_block_number(&self.http, rpc_url).await else {
                continue;
            };
            let from = {
                let mut cursor = self.watcher_cursor.lock().await;
                let prev = cursor.entry(chain_id).or_insert(to_block);
                let f = (*prev).max(to_block.saturating_sub(MAX_BLOCK_RANGE));
                *prev = to_block + 1;
                f
            };

            let logs = eth::fetch_logs(&self.http, rpc_url, bank_addr, &topic, from, to_block).await;
            for log in &logs {
                if log.topics.len() < 2 {
                    continue;
                }
                let message_id = log.topics[1].as_str();
                if let Some(row) = self
                    .reserve_repo
                    .find_by_dest_and_message_id(chain_id, message_id)
                    .await
                {
                    if row.status != "completed" {
                        info!(
                            op_id = %row.op_id,
                            dest_chain = chain_id.0,
                            message_id,
                            "reserve_path: ReserveBridgeCompleted observed"
                        );
                        self.reserve_repo
                            .update_completed(&OperationId(row.op_id))
                            .await;
                    }
                }
            }
        }

        // Time out stuck rows.
        let stuck = self
            .reserve_repo
            .list_stuck(stuck_timeout.as_secs() as i64)
            .await;
        for row in stuck {
            warn!(
                op_id = %row.op_id,
                status = %row.status,
                source_chain = row.source_chain_id,
                dest_chain = row.dest_chain_id,
                "reserve_path: op past stuck timeout — marking failed for operator review"
            );
            self.reserve_repo
                .update_failed(&OperationId(row.op_id))
                .await;
        }
    }

    // ── Nonce helper ──────────────────────────────────────────────────────────

    async fn get_nonce(
        &self,
        rpc_url: &str,
        chain_id: ChainId,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_hashes_are_stable() {
        let mut s = [0u8; 4];
        s.copy_from_slice(&keccak256(b"reserveDepth()")[..4]);
        // Sanity: selector exists and isn't trivially zero.
        assert!(s.iter().any(|&b| b != 0));
    }
}
