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
use k256::ecdsa::SigningKey;
use tokio::sync::{Mutex, RwLock};
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::domain::events::{InitiatedEvent, ReleasedEvent};
use crate::domain::newtypes::ChainId;
use crate::domain::repository::WatcherRepository;
use crate::domain::status::AlertType;
use crate::error::TxError;
use crate::eth;
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

// ── Watcher module ───────────────────────────────────────────────────────────

pub struct Watcher {
    watcher_repo: Arc<dyn WatcherRepository>,
    config: Arc<Config>,
    http: reqwest::Client,
    /// Pauser ECDSA signing key (absent when the key file is missing/invalid).
    pauser_key: Option<Arc<SigningKey>>,
    /// Ethereum address derived from the pauser signing key.
    pauser_address: Option<Address>,
    /// Per-chain nonce cache for pauser transactions.
    nonce_cache: Arc<Mutex<HashMap<ChainId, u64>>>,
    /// In-memory cache of HotPathInitiated events keyed by eventId.
    /// eventId is the bytes32 that the relayer passes as `sourceEventHash` to
    /// `releaseHotPath`, so it appears as `sourceEventHash` in HotPathReleased.
    initiated_cache: Arc<RwLock<HashMap<B256, InitiatedEvent>>>,
    /// keccak256("HotPathInitiated(address,address,uint256,uint256,bytes32,uint256)")
    hot_path_initiated_topic: B256,
    /// keccak256("HotPathReleased(address,uint256,bytes32)")
    hot_path_released_topic: B256,
    /// 4-byte selector for `pause()`
    pause_selector: [u8; 4],
}

impl Watcher {
    /// Construct an `Arc<Watcher>`.  Call `spawn_background` on the returned
    /// value to start the verification loops.
    pub fn new(
        watcher_repo: Arc<dyn WatcherRepository>,
        config: Arc<Config>,
        http: reqwest::Client,
    ) -> Arc<Self> {
        let (pauser_key, pauser_address) = match &config.pauser_key_path {
            Some(path) => eth::load_signing_key(path),
            None => (None, None),
        };
        if pauser_key.is_none() {
            warn!("watcher: pauser key not loaded — pause actions will be skipped");
        }

        let hot_path_initiated_topic =
            keccak256(b"HotPathInitiated(address,address,uint256,uint256,bytes32,uint256)");
        let hot_path_released_topic = keccak256(b"HotPathReleased(address,uint256,bytes32)");

        let pause_hash = keccak256(b"pause()");
        let mut pause_selector = [0u8; 4];
        pause_selector.copy_from_slice(&pause_hash[..4]);

        Arc::new(Self {
            watcher_repo,
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
        let limit = req.into_inner().limit.clamp(1, 100) as i64;
        let alert_ids = self
            .watcher_repo
            .get_alert_ids(limit)
            .await
            .map_err(|e| Status::internal(e))?;
        Ok(Response::new(GetWatcherAlertsResponse { alert_ids }))
    }

    // ── Background loops ──────────────────────────────────────────────────────

    /// Poll every chain for `HotPathInitiated` events and populate the
    /// in-memory cache keyed by `eventId`.
    ///
    /// This loop intentionally stays ahead of the released loop; the 1-second
    /// stagger in `poll_released_loop` gives it a head start on startup.
    async fn poll_initiated_loop(self: Arc<Self>) {
        let mut last_block: HashMap<ChainId, u64> = HashMap::new();
        info!("watcher: initiated-event cache polling started");

        loop {
            let chains: Vec<(ChainId, String, String)> = self
                .effective_rpc_urls()
                .iter()
                .filter_map(|(&chain_id, rpc_url)| {
                    self.config
                        .contract_addresses
                        .get(&chain_id)
                        .map(|addr| (ChainId(chain_id), rpc_url.clone(), addr.clone()))
                })
                .collect();

            for (chain_id, rpc_url, bank_addr) in chains {
                let to_block = match eth::fetch_block_number(&self.http, &rpc_url).await {
                    Some(b) => b,
                    None => continue,
                };

                let scan_from = match last_block.get(&chain_id) {
                    Some(&last) => last,
                    None => to_block, // first poll: only current block to avoid replay
                };
                let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

                let topic = format!("{}", self.hot_path_initiated_topic);
                let logs = eth::fetch_logs(
                    &self.http, &rpc_url, &bank_addr, &topic, scan_from, to_block,
                )
                .await;

                let mut cache = self.initiated_cache.write().await;
                for log in &logs {
                    if let Some((event_id, event)) = self.parse_initiated_event(log, chain_id) {
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

        let mut last_block: HashMap<ChainId, u64> = HashMap::new();
        info!("watcher: released-event verification polling started");

        loop {
            let chains: Vec<(ChainId, String, String)> = self
                .effective_rpc_urls()
                .iter()
                .filter_map(|(&chain_id, rpc_url)| {
                    self.config
                        .contract_addresses
                        .get(&chain_id)
                        .map(|addr| (ChainId(chain_id), rpc_url.clone(), addr.clone()))
                })
                .collect();

            for (chain_id, rpc_url, bank_addr) in chains {
                let to_block = match eth::fetch_block_number(&self.http, &rpc_url).await {
                    Some(b) => b,
                    None => continue,
                };

                let scan_from = match last_block.get(&chain_id) {
                    Some(&last) => last,
                    None => to_block,
                };
                let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

                let topic = format!("{}", self.hot_path_released_topic);
                let logs = eth::fetch_logs(
                    &self.http, &rpc_url, &bank_addr, &topic, scan_from, to_block,
                )
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
        if self.watcher_repo.already_verified(&transfer_id_hex).await {
            return;
        }

        let recipient_hex = format!("0x{}", eth::bytes_to_hex(release.recipient.as_slice()));

        let (alert_type, initiated_opt) = {
            let cache = self.initiated_cache.read().await;
            match cache.get(&release.transfer_id).cloned() {
                Some(initiated) => {
                    let alert_type = release.verify_against(&initiated);
                    (alert_type, Some(initiated))
                }
                None => (AlertType::SourceNotFound, None),
            }
        };

        // For anomalies: pause the contract *before* persisting the alert so
        // the pause tx hash is captured in the detail JSON.
        let pause_tx = if alert_type != AlertType::Verified {
            warn!(
                transfer_id = %transfer_id_hex,
                alert_type = %alert_type,
                dest_chain = release.dest_chain_id.0,
                dest_tx    = %release.dest_tx_hash,
                "watcher: anomaly detected — triggering pause"
            );
            self.pause_contract(&release).await
        } else {
            info!(
                transfer_id = %transfer_id_hex,
                dest_chain = release.dest_chain_id.0,
                "watcher: release verified"
            );
            None
        };

        // Build the detail JSON with full context for post-incident review.
        let detail = match initiated_opt.as_ref() {
            Some(init) => {
                let expected_recipient =
                    format!("0x{}", eth::bytes_to_hex(init.recipient.as_slice()));
                let mut obj = serde_json::json!({
                    "source_chain_id":    init.source_chain_id.0,
                    "dest_chain_id":      release.dest_chain_id.0,
                    "source_tx_hash":     init.source_tx_hash.as_str(),
                    "dest_tx_hash":       release.dest_tx_hash.as_str(),
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
                    "dest_chain_id":    release.dest_chain_id.0,
                    "dest_tx_hash":     release.dest_tx_hash.as_str(),
                    "actual_recipient": recipient_hex,
                    "actual_amount":    release.amount.to_string(),
                });
                if let Some(ref tx) = pause_tx {
                    obj["pause_tx_hash"] = serde_json::Value::String(tx.clone());
                }
                obj.to_string()
            }
        };

        self.watcher_repo
            .insert_alert(&transfer_id_hex, alert_type, &detail)
            .await;
    }

    /// Submit `pause()` on the Bank Contract for the release's destination
    /// chain.  Retries up to `MAX_PAUSE_RETRIES` times with exponential
    /// back-off.  Returns the pause transaction hash on success.
    async fn pause_contract(&self, release: &ReleasedEvent) -> Option<String> {
        let key = self.pauser_key.as_ref()?;
        let pauser_addr = self.pauser_address?;

        let rpc_url = self
            .effective_rpc_urls()
            .get(&release.dest_chain_id.0)
            .cloned()?;
        let bank_addr = self
            .config
            .contract_addresses
            .get(&release.dest_chain_id.0)
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
                        chain_id = chain_id.0,
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
            chain_id = chain_id.0,
            transfer_id = %release.transfer_id,
            "watcher: pause failed after all retries"
        );
        None
    }

    async fn submit_pause_once(
        &self,
        rpc_url: &str,
        bank_addr: &str,
        chain_id: ChainId,
        key: &SigningKey,
        pauser_addr: &Address,
    ) -> Result<String, TxError> {
        let nonce = self.get_nonce(rpc_url, chain_id, pauser_addr).await?;
        let (max_fee, max_priority_fee) = eth::fetch_gas_params(&self.http, rpc_url).await?;

        // pause() has no arguments — calldata is the 4-byte selector only.
        let call_data = self.pause_selector.to_vec();
        let bank_addr_bytes = eth::decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or(TxError::InvalidAddress)?;

        let raw_hex = eth::sign_eip1559_tx(
            chain_id.0,
            nonce,
            max_priority_fee,
            max_fee,
            PAUSE_GAS_LIMIT,
            &bank_addr_bytes,
            &[],
            &call_data,
            key,
        )?;

        // Advance cached nonce before sending.
        self.nonce_cache.lock().await.insert(chain_id, nonce + 1);

        let tx_hash = eth::send_raw_transaction(&self.http, rpc_url, &raw_hex).await?;
        eth::wait_for_receipt(&self.http, rpc_url, &tx_hash, Duration::from_secs(60)).await?;
        Ok(tx_hash)
    }

    // ── Nonce helper ─────────────────────────────────────────────────────────

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

    // ── Event parsing ─────────────────────────────────────────────────────────

    /// Parse a `HotPathInitiated` log.
    ///
    /// ABI layout (Bank.sol):
    ///   topics[0] = event selector
    ///   topics[1] = indexed sender   (address, left-padded to 32 bytes)
    ///   topics[2] = indexed to       (address, left-padded to 32 bytes)
    ///   data      = abi_encode(uint256 amount, uint256 destinationChainId,
    ///                          bytes32 eventHash, uint256 fee)
    ///              [0..32]     [32..64]     [64..96]     [96..128]
    fn parse_initiated_event(
        &self,
        log: &eth::RpcLog,
        chain_id: ChainId,
    ) -> Option<(B256, InitiatedEvent)> {
        if log.topics.len() < 3 {
            return None;
        }
        let recipient_raw = eth::decode_hex(&log.topics[2])?;
        if recipient_raw.len() < 32 {
            return None;
        }
        let recipient = Address::from_slice(&recipient_raw[12..32]);

        let data = eth::decode_hex(&log.data)?;
        // 4 slots: amount + destinationChainId + eventHash + fee
        if data.len() < 128 {
            return None;
        }
        let amount_bytes: [u8; 32] = data[0..32].try_into().ok()?;
        let amount = U256::from_be_bytes(amount_bytes);
        // eventHash is the third 32-byte slot in data.
        let event_id = B256::from_slice(&data[64..96]);

        use crate::domain::newtypes::TxHash;
        Some((
            event_id,
            InitiatedEvent {
                source_chain_id: chain_id,
                source_tx_hash: TxHash(log.transaction_hash.clone()),
                recipient,
                amount,
            },
        ))
    }

    /// Parse a `HotPathReleased` log.
    ///
    /// ABI layout (Bank.sol):
    ///   topics[0] = event selector
    ///   topics[1] = indexed to        (address, left-padded to 32 bytes)
    ///   topics[2] = indexed sourceEventHash (bytes32)
    ///   data      = abi_encode(uint256 amount)
    fn parse_released_event(&self, log: &eth::RpcLog, chain_id: ChainId) -> Option<ReleasedEvent> {
        if log.topics.len() < 3 {
            return None;
        }

        let recipient_raw = eth::decode_hex(&log.topics[1])?;
        if recipient_raw.len() < 32 {
            return None;
        }
        let recipient = Address::from_slice(&recipient_raw[12..32]);

        let transfer_id_raw = eth::decode_hex(&log.topics[2])?;
        if transfer_id_raw.len() < 32 {
            return None;
        }
        let transfer_id = B256::from_slice(&transfer_id_raw[..32]);

        let data = eth::decode_hex(&log.data)?;
        if data.len() < 32 {
            return None;
        }
        let amount_bytes: [u8; 32] = data[0..32].try_into().ok()?;
        let amount = U256::from_be_bytes(amount_bytes);

        use crate::domain::newtypes::TxHash;
        Some(ReleasedEvent {
            dest_chain_id: chain_id,
            dest_tx_hash: TxHash(log.transaction_hash.clone()),
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
            Some(m) => m,
            None => &self.config.rpc_urls,
        }
    }
}
