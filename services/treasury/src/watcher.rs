//! Watcher / independent verifier module (Task 06).
//!
//! Independently verifies every hot-path release against the corresponding
//! source-chain event, and triggers an emergency `pause()` on the affected
//! Bank Contract whenever a mismatch or missing source event is detected.
//!
//! # Design
//!
//! The watcher runs two independent background loops:
//!
//! 1. **Initiated cache loop** — continuously polls `HotPathInitiated` events
//!    on all configured chains and caches them keyed by `eventId` (the bytes32
//!    that becomes `transferId` in `HotPathReleased` on the destination chain).
//!    Uses `WATCHER_RPC_URLS` (if configured) so it reads from different
//!    endpoints than the relayer.
//!
//! 2. **Released verification loop** — polls `HotPathReleased` events on all
//!    chains.  For each release it:
//!    - Looks up the source event in the in-memory initiated cache.
//!    - Classifies the result as `Verified`, `Mismatch`, or `SourceNotFound`.
//!    - Writes the classification to `treasury.watcher_alerts`.
//!    - On `Mismatch` or `SourceNotFound` calls `pause()` on the affected Bank
//!      Contract using the PAUSER_ROLE key and records the pause tx hash.
//!
//! # Independence from the relayer
//!
//! The watcher does **not** read from `relay_logs`; it re-derives source events
//! directly from chain RPC calls.  Setting `WATCHER_RPC_URLS` to a different
//! provider than `RPC_URLS` ensures the watcher continues operating even if
//! the relayer's RPC provider is compromised.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, Address, B256, U256};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, Signature, SigningKey};
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::proto::treasury::{GetWatcherAlertsRequest, GetWatcherAlertsResponse};

// ── Constants ────────────────────────────────────────────────────────────────

/// How often each chain is polled for new events.
const WATCHER_POLL_INTERVAL: Duration = Duration::from_secs(3);
/// Maximum block range per `eth_getLogs` call (matches relayer limit).
const MAX_BLOCK_RANGE: u64 = 2_000;
/// Retry budget for `pause()` transactions with exponential back-off.
const MAX_PAUSE_RETRIES: u32 = 3;
/// Conservative gas limit for `pause()` — no storage writes beyond the flag.
const PAUSE_GAS_LIMIT: u64 = 80_000;

// ── Internal event types ──────────────────────────────────────────────────────

/// A cached `HotPathInitiated` event from a source chain.
#[derive(Debug, Clone)]
struct InitiatedEvent {
    source_chain_id: u64,
    source_tx_hash: String,
    recipient: Address,
    amount: U256,
}

/// A `HotPathReleased` event observed on a destination chain.
#[derive(Debug, Clone)]
struct ReleasedEvent {
    dest_chain_id: u64,
    dest_tx_hash: String,
    /// transferId == eventId from the corresponding HotPathInitiated.
    transfer_id: B256,
    recipient: Address,
    amount: U256,
}

// ── JSON-RPC log entry ───────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct RpcLog {
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    topics: Vec<String>,
    data: String,
}

// ── Watcher module ───────────────────────────────────────────────────────────

pub struct Watcher {
    pool: PgPool,
    config: Arc<Config>,
    http: reqwest::Client,
    /// Pauser ECDSA signing key (absent when the key file is missing/invalid).
    pauser_key: Option<Arc<SigningKey>>,
    /// Ethereum address derived from the pauser signing key.
    pauser_address: Option<Address>,
    /// Per-chain nonce cache for pauser transactions.
    nonce_cache: Arc<Mutex<HashMap<u64, u64>>>,
    /// In-memory cache of HotPathInitiated events keyed by eventId.
    /// eventId is the bytes32 that the relayer passes as `transferId` to
    /// `hotPathRelease`, so it appears as `transferId` in HotPathReleased.
    initiated_cache: Arc<RwLock<HashMap<B256, InitiatedEvent>>>,
    /// keccak256("HotPathInitiated(address,address,uint256,uint64,bytes32)")
    hot_path_initiated_topic: B256,
    /// keccak256("HotPathReleased(bytes32,address,uint256)")
    hot_path_released_topic: B256,
    /// 4-byte selector for `pause()`
    pause_selector: [u8; 4],
}

impl Watcher {
    /// Construct an `Arc<Watcher>`.  Call `spawn_background` on the returned
    /// value to start the verification loops.
    pub fn new(pool: PgPool, config: Arc<Config>, http: reqwest::Client) -> Arc<Self> {
        let (pauser_key, pauser_address) = match &config.pauser_key_path {
            Some(path) => load_signing_key(path),
            None => (None, None),
        };
        if pauser_key.is_none() {
            warn!("watcher: pauser key not loaded — pause actions will be skipped");
        }

        let hot_path_initiated_topic =
            keccak256(b"HotPathInitiated(address,address,uint256,uint64,bytes32)");
        let hot_path_released_topic = keccak256(b"HotPathReleased(bytes32,address,uint256)");

        let pause_hash = keccak256(b"pause()");
        let mut pause_selector = [0u8; 4];
        pause_selector.copy_from_slice(&pause_hash[..4]);

        Arc::new(Self {
            pool,
            config,
            http,
            pauser_key,
            pauser_address,
            nonce_cache: Arc::new(Mutex::new(HashMap::new())),
            initiated_cache: Arc::new(RwLock::new(HashMap::new())),
            hot_path_initiated_topic,
            hot_path_released_topic,
            pause_selector,
        })
    }

    /// Spawn the initiated-event caching loop and the released-event
    /// verification loop as independent tokio tasks.
    pub fn spawn_background(self: Arc<Self>) {
        let this = Arc::clone(&self);
        tokio::spawn(async move { this.poll_initiated_loop().await });
        tokio::spawn(async move { self.poll_released_loop().await });
    }

    // ── gRPC handler ─────────────────────────────────────────────────────────

    pub async fn get_watcher_alerts(
        &self,
        req: Request<GetWatcherAlertsRequest>,
    ) -> Result<Response<GetWatcherAlertsResponse>, Status> {
        let limit = req.into_inner().limit.max(1).min(100) as i64;
        let rows = sqlx::query(
            "SELECT id FROM treasury.watcher_alerts \
             ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        use sqlx::Row;
        let alert_ids = rows
            .iter()
            .map(|r| r.try_get::<i64, _>("id").unwrap_or(0).to_string())
            .collect();

        Ok(Response::new(GetWatcherAlertsResponse { alert_ids }))
    }

    // ── Background loops ──────────────────────────────────────────────────────

    /// Poll every chain for `HotPathInitiated` events and populate the
    /// in-memory cache keyed by `eventId`.
    ///
    /// This loop intentionally stays ahead of the released loop; the 1-second
    /// stagger in `poll_released_loop` gives it a head start on startup.
    async fn poll_initiated_loop(self: Arc<Self>) {
        let mut last_block: HashMap<u64, u64> = HashMap::new();
        info!("watcher: initiated-event cache polling started");

        loop {
            let chains: Vec<(u64, String, String)> = self
                .effective_rpc_urls()
                .iter()
                .filter_map(|(&chain_id, rpc_url)| {
                    self.config
                        .contract_addresses
                        .get(&chain_id)
                        .map(|addr| (chain_id, rpc_url.clone(), addr.clone()))
                })
                .collect();

            for (chain_id, rpc_url, bank_addr) in chains {
                let to_block = match self.fetch_block_number(&rpc_url).await {
                    Some(b) => b,
                    None => continue,
                };

                let scan_from = match last_block.get(&chain_id) {
                    Some(&last) => last,
                    None => to_block, // first poll: only current block to avoid replay
                };
                let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

                let topic = format!("{}", self.hot_path_initiated_topic);
                let logs = self
                    .fetch_logs(&rpc_url, &bank_addr, &topic, scan_from, to_block)
                    .await;

                let mut cache = self.initiated_cache.write().await;
                for log in &logs {
                    if let Some((event_id, event)) =
                        self.parse_initiated_event(log, chain_id)
                    {
                        cache.insert(event_id, event);
                    }
                }
                drop(cache);

                last_block.insert(chain_id, to_block + 1);
            }

            tokio::time::sleep(WATCHER_POLL_INTERVAL).await;
        }
    }

    /// Poll every chain for `HotPathReleased` events and verify each one
    /// against the initiated-event cache.
    async fn poll_released_loop(self: Arc<Self>) {
        // Let the initiated loop warm the cache before verification starts.
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut last_block: HashMap<u64, u64> = HashMap::new();
        info!("watcher: released-event verification polling started");

        loop {
            let chains: Vec<(u64, String, String)> = self
                .effective_rpc_urls()
                .iter()
                .filter_map(|(&chain_id, rpc_url)| {
                    self.config
                        .contract_addresses
                        .get(&chain_id)
                        .map(|addr| (chain_id, rpc_url.clone(), addr.clone()))
                })
                .collect();

            for (chain_id, rpc_url, bank_addr) in chains {
                let to_block = match self.fetch_block_number(&rpc_url).await {
                    Some(b) => b,
                    None => continue,
                };

                let scan_from = match last_block.get(&chain_id) {
                    Some(&last) => last,
                    None => to_block,
                };
                let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

                let topic = format!("{}", self.hot_path_released_topic);
                let logs = self
                    .fetch_logs(&rpc_url, &bank_addr, &topic, scan_from, to_block)
                    .await;

                for log in logs {
                    if let Some(release) = self.parse_released_event(&log, chain_id) {
                        let watcher = Arc::clone(&self);
                        tokio::spawn(async move { watcher.verify_release(release).await });
                    }
                }

                last_block.insert(chain_id, to_block + 1);
            }

            tokio::time::sleep(WATCHER_POLL_INTERVAL).await;
        }
    }

    // ── Verification core ─────────────────────────────────────────────────────

    /// Verify a single `HotPathReleased` event against the initiated cache.
    ///
    /// Outcome classification:
    /// - `Verified`      — source event found; amount and recipient match.
    /// - `Mismatch`      — source event found; amount or recipient differs.
    /// - `SourceNotFound`— no `HotPathInitiated` event with this `transferId`.
    ///
    /// `Mismatch` and `SourceNotFound` both trigger a `pause()` call before the
    /// alert is persisted so the pause tx hash can be included in the detail.
    async fn verify_release(&self, release: ReleasedEvent) {
        let transfer_id_hex = format!("{}", release.transfer_id);

        // Idempotency: skip releases we've already recorded.
        if self.already_verified(&transfer_id_hex).await {
            return;
        }

        let recipient_hex = format!("0x{}", bytes_to_hex_raw(release.recipient.as_slice()));

        let (alert_type, initiated_opt) = {
            let cache = self.initiated_cache.read().await;
            match cache.get(&release.transfer_id).cloned() {
                Some(initiated) => {
                    let amount_ok = initiated.amount == release.amount;
                    let recipient_ok = initiated.recipient == release.recipient;
                    if amount_ok && recipient_ok {
                        ("Verified", Some(initiated))
                    } else {
                        ("Mismatch", Some(initiated))
                    }
                }
                None => ("SourceNotFound", None),
            }
        };

        // For anomalies: pause the contract *before* persisting the alert so
        // the pause tx hash is captured in the detail JSON.
        let pause_tx = if alert_type != "Verified" {
            warn!(
                transfer_id = %transfer_id_hex,
                alert_type,
                dest_chain = release.dest_chain_id,
                dest_tx    = %release.dest_tx_hash,
                "watcher: anomaly detected — triggering pause"
            );
            self.pause_contract(&release).await
        } else {
            info!(
                transfer_id = %transfer_id_hex,
                dest_chain = release.dest_chain_id,
                "watcher: release verified"
            );
            None
        };

        // Build the detail JSON with full context for post-incident review.
        let detail = match initiated_opt.as_ref() {
            Some(init) => {
                let expected_recipient =
                    format!("0x{}", bytes_to_hex_raw(init.recipient.as_slice()));
                let mut obj = serde_json::json!({
                    "source_chain_id":    init.source_chain_id,
                    "dest_chain_id":      release.dest_chain_id,
                    "source_tx_hash":     init.source_tx_hash,
                    "dest_tx_hash":       release.dest_tx_hash,
                    "expected_recipient": expected_recipient,
                    "actual_recipient":   recipient_hex,
                    "expected_amount":    init.amount.to_string(),
                    "actual_amount":      release.amount.to_string(),
                });
                if let Some(ref tx) = pause_tx {
                    obj["pause_tx_hash"] = serde_json::Value::String(tx.clone());
                }
                obj.to_string()
            }
            None => {
                let mut obj = serde_json::json!({
                    "dest_chain_id":    release.dest_chain_id,
                    "dest_tx_hash":     release.dest_tx_hash,
                    "actual_recipient": recipient_hex,
                    "actual_amount":    release.amount.to_string(),
                });
                if let Some(ref tx) = pause_tx {
                    obj["pause_tx_hash"] = serde_json::Value::String(tx.clone());
                }
                obj.to_string()
            }
        };

        self.insert_alert(&transfer_id_hex, alert_type, &detail).await;
    }

    /// Submit `pause()` on the Bank Contract for the release's destination
    /// chain.  Retries up to `MAX_PAUSE_RETRIES` times with exponential
    /// back-off.  Returns the pause transaction hash on success.
    async fn pause_contract(&self, release: &ReleasedEvent) -> Option<String> {
        let key = self.pauser_key.as_ref()?;
        let pauser_addr = self.pauser_address?;

        let rpc_url = self
            .effective_rpc_urls()
            .get(&release.dest_chain_id)
            .cloned()?;
        let bank_addr = self
            .config
            .contract_addresses
            .get(&release.dest_chain_id)
            .cloned()?;

        let chain_id = release.dest_chain_id;
        let mut delay = Duration::from_secs(1);

        for attempt in 1..=MAX_PAUSE_RETRIES {
            match self
                .submit_pause_once(&rpc_url, &bank_addr, chain_id, key, &pauser_addr)
                .await
            {
                Ok(tx_hash) => {
                    info!(
                        transfer_id = %release.transfer_id,
                        chain_id,
                        pause_tx = %tx_hash,
                        "watcher: contract paused"
                    );
                    return Some(tx_hash);
                }
                Err(e) => {
                    warn!(attempt, err = %e, "watcher: pause attempt failed");
                    // Flush cached nonce so the next attempt fetches fresh.
                    self.nonce_cache.lock().await.remove(&chain_id);
                    if attempt < MAX_PAUSE_RETRIES {
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                    }
                }
            }
        }

        error!(
            chain_id,
            transfer_id = %release.transfer_id,
            "watcher: pause failed after all retries"
        );
        None
    }

    async fn submit_pause_once(
        &self,
        rpc_url: &str,
        bank_addr: &str,
        chain_id: u64,
        key: &SigningKey,
        pauser_addr: &Address,
    ) -> Result<String, String> {
        let nonce = self.next_nonce(rpc_url, chain_id, pauser_addr).await?;
        let (max_fee, max_priority_fee) = self.fetch_gas_params(rpc_url).await?;

        // pause() has no arguments — calldata is the 4-byte selector only.
        let call_data = self.pause_selector.to_vec();
        let bank_addr_bytes = decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or("invalid bank contract address")?;

        // EIP-1559 signing payload: 0x02 || rlp([chain_id, nonce, ...])
        let signing_rlp = build_tx_rlp(
            chain_id,
            nonce,
            max_priority_fee,
            max_fee,
            PAUSE_GAS_LIMIT,
            &bank_addr_bytes,
            &call_data,
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
        let signed_rlp = rlp_encode_list(&[
            rlp_encode_uint(chain_id),
            rlp_encode_uint(nonce),
            rlp_encode_uint(max_priority_fee),
            rlp_encode_uint(max_fee),
            rlp_encode_uint(PAUSE_GAS_LIMIT),
            rlp_encode_bytes(&bank_addr_bytes),
            rlp_encode_bytes(&[]), // value = 0
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
        self.wait_for_receipt(rpc_url, &tx_hash).await?;
        Ok(tx_hash)
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
        let resp: serde_json::Value = match self
            .http
            .post(rpc_url)
            .json(&body)
            .send()
            .await
        {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(_) => return vec![],
            },
            Err(_) => return vec![],
        };
        serde_json::from_value(resp["result"].clone()).unwrap_or_default()
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
        let gp = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|e| e.to_string())?;
        let tip = gp / 10;
        Ok((gp + tip, tip)) // (maxFeePerGas, maxPriorityFeePerGas)
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
            return Err(format!("eth_sendRawTransaction: {}", err));
        }
        resp["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no tx hash in response".to_string())
    }

    async fn wait_for_receipt(&self, rpc_url: &str, tx_hash: &str) -> Result<(), String> {
        const POLL: Duration = Duration::from_millis(500);
        const TIMEOUT: Duration = Duration::from_secs(60);
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1
        });
        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(format!("timed out waiting for receipt: {}", tx_hash));
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
                    Some("0x1") => return Ok(()),
                    Some("0x0") => return Err(format!("transaction reverted: {}", tx_hash)),
                    _ => {}
                }
            }
            tokio::time::sleep(POLL).await;
        }
    }

    // ── Database helpers ──────────────────────────────────────────────────────

    /// Returns true if a watcher alert already exists for this transferId.
    async fn already_verified(&self, transfer_id_hex: &str) -> bool {
        sqlx::query(
            "SELECT 1 FROM treasury.watcher_alerts WHERE source_event_hash = $1",
        )
        .bind(transfer_id_hex)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
    }

    /// Persist a verification result.  The unique index on `source_event_hash`
    /// makes the insert idempotent via ON CONFLICT DO NOTHING.
    async fn insert_alert(&self, transfer_id_hex: &str, alert_type: &str, detail: &str) {
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO treasury.watcher_alerts (source_event_hash, alert_type, detail)
            VALUES ($1, $2, $3)
            ON CONFLICT (source_event_hash) DO NOTHING
            "#,
        )
        .bind(transfer_id_hex)
        .bind(alert_type)
        .bind(detail)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "watcher: failed to insert alert");
        }
    }

    // ── Event parsing ─────────────────────────────────────────────────────────

    /// Parse a `HotPathInitiated` log.
    ///
    /// ABI layout:
    ///   topics[0] = event selector
    ///   topics[1] = indexed sender   (address, left-padded to 32 bytes)
    ///   topics[2] = indexed recipient(address, left-padded to 32 bytes)
    ///   data      = abi_encode(uint256 amount, uint64 destChainId, bytes32 eventId)
    ///                         [0..32]           [32..64]            [64..96]
    fn parse_initiated_event(
        &self,
        log: &RpcLog,
        chain_id: u64,
    ) -> Option<(B256, InitiatedEvent)> {
        if log.topics.len() < 3 {
            return None;
        }
        let recipient_raw = decode_hex(&log.topics[2])?;
        if recipient_raw.len() < 32 {
            return None;
        }
        let recipient = Address::from_slice(&recipient_raw[12..32]);

        let data = decode_hex(&log.data)?;
        if data.len() < 96 {
            return None;
        }
        let amount_bytes: [u8; 32] = data[0..32].try_into().ok()?;
        let amount = U256::from_be_bytes(amount_bytes);
        // eventId is the third 32-byte slot in data.
        let event_id = B256::from_slice(&data[64..96]);

        Some((
            event_id,
            InitiatedEvent {
                source_chain_id: chain_id,
                source_tx_hash: log.transaction_hash.clone(),
                recipient,
                amount,
            },
        ))
    }

    /// Parse a `HotPathReleased` log.
    ///
    /// ABI layout:
    ///   topics[0] = event selector
    ///   topics[1] = indexed transferId (bytes32)
    ///   topics[2] = indexed recipient  (address, left-padded to 32 bytes)
    ///   data      = abi_encode(uint256 amount)
    fn parse_released_event(&self, log: &RpcLog, chain_id: u64) -> Option<ReleasedEvent> {
        if log.topics.len() < 3 {
            return None;
        }
        let transfer_id_raw = decode_hex(&log.topics[1])?;
        if transfer_id_raw.len() < 32 {
            return None;
        }
        let transfer_id = B256::from_slice(&transfer_id_raw[..32]);

        let recipient_raw = decode_hex(&log.topics[2])?;
        if recipient_raw.len() < 32 {
            return None;
        }
        let recipient = Address::from_slice(&recipient_raw[12..32]);

        let data = decode_hex(&log.data)?;
        if data.len() < 32 {
            return None;
        }
        let amount_bytes: [u8; 32] = data[0..32].try_into().ok()?;
        let amount = U256::from_be_bytes(amount_bytes);

        Some(ReleasedEvent {
            dest_chain_id: chain_id,
            dest_tx_hash: log.transaction_hash.clone(),
            transfer_id,
            recipient,
            amount,
        })
    }

    // ── Config helpers ────────────────────────────────────────────────────────

    /// Return the effective RPC URL map for the watcher.
    ///
    /// Uses `WATCHER_RPC_URLS` when configured so the watcher reads from
    /// different endpoints than the relayer.  Falls back to `RPC_URLS`.
    fn effective_rpc_urls(&self) -> &HashMap<u64, String> {
        match &self.config.watcher_rpc_urls {
            Some(m) => &**m,
            None => &*self.config.rpc_urls,
        }
    }
}

// ── Standalone helpers ────────────────────────────────────────────────────────

/// Load a signing key from a hex-encoded file and derive its Ethereum address.
fn load_signing_key(path: &str) -> (Option<Arc<SigningKey>>, Option<Address>) {
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

fn bytes_to_hex_raw(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

fn rlp_encode_uint(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![0x80]; // RLP encoding of zero is the empty string
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

fn build_tx_rlp(
    chain_id: u64,
    nonce: u64,
    max_priority_fee: u64,
    max_fee: u64,
    gas_limit: u64,
    to: &[u8],
    data: &[u8],
) -> Vec<u8> {
    rlp_encode_list(&[
        rlp_encode_uint(chain_id),
        rlp_encode_uint(nonce),
        rlp_encode_uint(max_priority_fee),
        rlp_encode_uint(max_fee),
        rlp_encode_uint(gas_limit),
        rlp_encode_bytes(to),
        rlp_encode_bytes(&[]), // value = 0
        rlp_encode_bytes(data),
        rlp_encode_list(&[]), // empty access list
    ])
}
