//! Hot-path relay module.
//!
//! Listens for `HotPathInitiated` events on every source chain, validates the
//! destination chain is active (via `RouteReceiver.sol` `ActivationPublished`
//! events) and that the destination pool has sufficient depth, then submits
//! `releaseHotPath` on the destination chain and records the outcome in
//! `treasury.relay_logs`.
//!
//! # Assumed Bank Contract interface (not yet deployed)
//!
//! Event:
//!   `HotPathInitiated(address indexed sender, address indexed recipient,
//!                     uint256 amount, uint64 destChainId, bytes32 eventId)`
//!
//! Write:
//!   `releaseHotPath(address recipient, uint256 amount, bytes32 sourceEventId)`
//!
//! Read:
//!   `poolDepth() returns (uint256)`

use std::collections::{HashMap, HashSet};
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
use crate::proto::treasury::{GetRelayStatusRequest, GetRelayStatusResponse};

// ── Constants ────────────────────────────────────────────────────────────────

const EVENT_POLL_INTERVAL: Duration = Duration::from_secs(2);
const ROUTE_RECEIVER_POLL_INTERVAL: Duration = Duration::from_secs(30);
const TX_RECEIPT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_TX_WAIT: Duration = Duration::from_secs(60);
const MAX_RELAY_RETRIES: u32 = 3;
/// Maximum block range per `eth_getLogs` call (some RPCs cap this).
const MAX_BLOCK_RANGE: u64 = 2_000;

// ── Parsed event ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct HotPathEvent {
    source_chain_id: u64,
    /// Transaction hash of the log — used as idempotency key in relay_logs.
    source_event_hash: String,
    #[allow(dead_code)] // logged via Debug; available for future watcher cross-reference
    sender: String,
    recipient: Address,
    amount: U256,
    dest_chain_id: u64,
    event_id: B256,
}

// ── JSON-RPC log entry ───────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct RpcLog {
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    topics: Vec<String>,
    data: String,
}

// ── Hot path module ──────────────────────────────────────────────────────────

pub struct HotPath {
    pool: PgPool,
    config: Arc<Config>,
    http: reqwest::Client,
    /// Chain IDs currently in the active set per the latest ActivationPublished
    /// event. Seeded with all configured chains at startup so the relay works
    /// before the first CRE publish arrives.
    active_chains: Arc<RwLock<HashSet<u64>>>,
    /// Relayer ECDSA signing key (absent when the key file is missing/invalid).
    relayer_key: Option<Arc<SigningKey>>,
    /// Ethereum address derived from the relayer signing key.
    relayer_address: Option<Address>,
    /// Per-chain nonce cache to avoid an extra RPC round-trip on each tx.
    nonce_cache: Arc<Mutex<HashMap<u64, u64>>>,
    /// keccak256("HotPathInitiated(address,address,uint256,uint64,bytes32)")
    hot_path_topic: B256,
    /// keccak256("ActivationPublished(string,string,string,uint256,string,string,uint256)")
    activation_topic: B256,
    /// 4-byte selector for `releaseHotPath(address,uint256,bytes32)`
    release_selector: [u8; 4],
    /// 4-byte selector for `poolDepth()`
    pool_depth_selector: [u8; 4],
}

impl HotPath {
    /// Construct an `Arc<HotPath>`. Call `spawn_background` on the returned
    /// value to start the event-listener and route-receiver loops.
    pub fn new(pool: PgPool, config: Arc<Config>, http: reqwest::Client) -> Arc<Self> {
        let (relayer_key, relayer_address) = load_relayer_key(&config.relayer_key_path);

        // Seed active chains with every chain that has an RPC URL so the relay
        // can forward events before the first ActivationPublished arrives.
        let initial: HashSet<u64> = config.rpc_urls.keys().cloned().collect();

        let hot_path_topic =
            keccak256(b"HotPathInitiated(address,address,uint256,uint64,bytes32)");
        let activation_topic = keccak256(
            b"ActivationPublished(string,string,string,uint256,string,string,uint256)",
        );

        let release_hash = keccak256(b"releaseHotPath(address,uint256,bytes32)");
        let pool_depth_hash = keccak256(b"poolDepth()");
        let mut release_selector = [0u8; 4];
        release_selector.copy_from_slice(&release_hash[..4]);
        let mut pool_depth_selector = [0u8; 4];
        pool_depth_selector.copy_from_slice(&pool_depth_hash[..4]);

        Arc::new(Self {
            pool,
            config,
            http,
            active_chains: Arc::new(RwLock::new(initial)),
            relayer_key,
            relayer_address,
            nonce_cache: Arc::new(Mutex::new(HashMap::new())),
            hot_path_topic,
            activation_topic,
            release_selector,
            pool_depth_selector,
        })
    }

    /// Spawn the route-receiver monitor and per-chain event-polling loops.
    /// Takes `Arc<Self>` so each background task holds a strong reference.
    pub fn spawn_background(self: Arc<Self>) {
        let this = Arc::clone(&self);
        tokio::spawn(async move { this.poll_route_receiver_loop().await });
        tokio::spawn(async move { self.poll_events_loop().await });
    }

    // ── gRPC handler ─────────────────────────────────────────────────────────

    pub async fn get_relay_status(
        &self,
        req: Request<GetRelayStatusRequest>,
    ) -> Result<Response<GetRelayStatusResponse>, Status> {
        let hash = req.into_inner().source_event_hash;
        let row = sqlx::query(
            "SELECT status FROM treasury.relay_logs WHERE source_event_hash = $1",
        )
        .bind(&hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        match row {
            Some(r) => {
                use sqlx::Row;
                let status: String = r.try_get("status").unwrap_or_default();
                Ok(Response::new(GetRelayStatusResponse { status }))
            }
            None => Err(Status::not_found("no relay log found for that event hash")),
        }
    }

    // ── Background loops ──────────────────────────────────────────────────────

    /// Poll every source chain for `HotPathInitiated` events and dispatch each
    /// to `relay_event`.
    async fn poll_events_loop(self: Arc<Self>) {
        let mut last_block: HashMap<u64, u64> = HashMap::new();
        info!("hot_path: event polling started");

        loop {
            // Collect chain entries to avoid borrow conflicts.
            let chains: Vec<(u64, String, String)> = self
                .config
                .rpc_urls
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

                let from_block = last_block.get(&chain_id).copied();
                let scan_from = match from_block {
                    Some(last) => last,
                    // First poll: only look at the current block to avoid
                    // replaying stale events from before startup.
                    None => to_block,
                };
                // Clamp to avoid oversized queries on RPCs that cap block ranges.
                let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

                let topic = format!("{}", self.hot_path_topic);
                let logs = self
                    .fetch_logs(&rpc_url, &bank_addr, &topic, scan_from, to_block)
                    .await;

                for log in logs {
                    if let Some(event) = self.parse_hot_path_event(&log, chain_id) {
                        let relay = Arc::clone(&self);
                        tokio::spawn(async move { relay.relay_event(event).await });
                    }
                }

                last_block.insert(chain_id, to_block + 1);
            }

            tokio::time::sleep(EVENT_POLL_INTERVAL).await;
        }
    }

    /// Poll `RouteReceiver.sol` for `ActivationPublished` events and update the
    /// in-memory active-chain set.
    async fn poll_route_receiver_loop(self: Arc<Self>) {
        // RouteReceiver is deployed on one chain; use any configured RPC
        // (the contract_address config maps to bank contracts per chain, while
        // route_receiver_address is global — we use the first available RPC).
        let rpc_url = match self.config.rpc_urls.values().next() {
            Some(u) => u.clone(),
            None => {
                warn!("hot_path: no RPC URLs — route receiver polling disabled");
                return;
            }
        };

        let mut last_block: u64 = 0;
        let topic = format!("{}", self.activation_topic);
        info!("hot_path: route receiver polling started");

        loop {
            let to_block = match self.fetch_block_number(&rpc_url).await {
                Some(b) => b,
                None => {
                    tokio::time::sleep(ROUTE_RECEIVER_POLL_INTERVAL).await;
                    continue;
                }
            };

            let scan_from = if last_block == 0 { to_block } else { last_block };

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
                    info!(chains = ?chains, "hot_path: activation state updated from RouteReceiver");
                    *self.active_chains.write().await = chains;
                }
            }

            last_block = to_block + 1;
            tokio::time::sleep(ROUTE_RECEIVER_POLL_INTERVAL).await;
        }
    }

    // ── Core relay logic ──────────────────────────────────────────────────────

    async fn relay_event(&self, event: HotPathEvent) {
        // 1. Idempotency guard.
        if self.relay_already_recorded(&event.source_event_hash).await {
            return;
        }

        // 2. Destination chain must be active.
        if !self
            .active_chains
            .read()
            .await
            .contains(&event.dest_chain_id)
        {
            warn!(
                src = event.source_event_hash,
                dest_chain = event.dest_chain_id,
                "hot_path: rejected — destination chain not active"
            );
            self.insert_relay_log(&event, None, "rejected_inactive_chain")
                .await;
            return;
        }

        // 3. Destination pool must have sufficient depth.
        let dest_rpc = match self.config.rpc_urls.get(&event.dest_chain_id) {
            Some(u) => u.clone(),
            None => {
                warn!(chain = event.dest_chain_id, "hot_path: no RPC for dest chain");
                return;
            }
        };
        let dest_bank = match self.config.contract_addresses.get(&event.dest_chain_id) {
            Some(a) => a.clone(),
            None => {
                warn!(
                    chain = event.dest_chain_id,
                    "hot_path: no bank contract for dest chain"
                );
                return;
            }
        };

        match self.fetch_pool_depth(&dest_rpc, &dest_bank).await {
            Some(depth) if depth >= event.amount => {} // sufficient — proceed
            Some(depth) => {
                warn!(
                    src = event.source_event_hash,
                    dest_chain = event.dest_chain_id,
                    %event.amount,
                    %depth,
                    "hot_path: rejected — insufficient pool depth"
                );
                self.insert_relay_log(&event, None, "rejected_insufficient_depth")
                    .await;
                return;
            }
            None => {
                warn!(
                    src = event.source_event_hash,
                    "hot_path: could not fetch pool depth — aborting relay"
                );
                return;
            }
        }

        // 4. Record pending.
        self.insert_relay_log(&event, None, "pending").await;

        // 5. Submit with retry.
        match self
            .submit_release_with_retry(&dest_rpc, &dest_bank, &event)
            .await
        {
            Ok(tx_hash) => {
                info!(
                    src = event.source_event_hash,
                    dest_tx = tx_hash,
                    "hot_path: relay completed"
                );
                self.update_relay_log(&event.source_event_hash, &tx_hash, "completed")
                    .await;
            }
            Err(e) => {
                error!(
                    src = event.source_event_hash,
                    err = %e,
                    "hot_path: relay failed after retries"
                );
                self.update_relay_log_failed(&event.source_event_hash).await;
            }
        }
    }

    async fn submit_release_with_retry(
        &self,
        rpc_url: &str,
        bank_addr: &str,
        event: &HotPathEvent,
    ) -> Result<String, String> {
        let key = self
            .relayer_key
            .as_ref()
            .ok_or_else(|| "relayer key not loaded".to_string())?;

        let chain_id = event.dest_chain_id;
        let mut delay = Duration::from_secs(1);

        for attempt in 1..=MAX_RELAY_RETRIES {
            match self
                .submit_release_once(rpc_url, bank_addr, event, key, chain_id)
                .await
            {
                Ok(tx_hash) => return Ok(tx_hash),
                Err(e) => {
                    warn!(
                        attempt,
                        err = %e,
                        "hot_path: relay attempt failed"
                    );
                    // Flush cached nonce so the next attempt fetches fresh.
                    self.nonce_cache.lock().await.remove(&chain_id);
                    if attempt < MAX_RELAY_RETRIES {
                        tokio::time::sleep(delay).await;
                        delay *= 2;
                    }
                }
            }
        }

        Err(format!("relay failed after {} attempts", MAX_RELAY_RETRIES))
    }

    async fn submit_release_once(
        &self,
        rpc_url: &str,
        bank_addr: &str,
        event: &HotPathEvent,
        key: &SigningKey,
        chain_id: u64,
    ) -> Result<String, String> {
        let relayer_addr = self
            .relayer_address
            .ok_or_else(|| "relayer address not set".to_string())?;

        let nonce = self.next_nonce(rpc_url, chain_id, &relayer_addr).await?;
        let (max_fee, max_priority_fee) = self.fetch_gas_params(rpc_url).await?;
        let gas_limit: u64 = 120_000; // conservative estimate for one storage write

        let call_data = encode_release_hot_path(
            &self.release_selector,
            &event.recipient,
            &event.amount,
            &event.event_id,
        );

        let bank_addr_bytes = decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or("invalid bank contract address")?;

        // EIP-1559 signing payload: 0x02 || rlp([chain_id, nonce, ...])
        let signing_rlp = build_tx_rlp(
            chain_id, nonce, max_priority_fee, max_fee, gas_limit,
            &bank_addr_bytes, &call_data,
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
            rlp_encode_uint(gas_limit),
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

        // Advance cached nonce before sending so concurrent calls don't reuse it.
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
        let deadline = tokio::time::Instant::now() + MAX_TX_WAIT;
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
                    Some("0x0") => {
                        return Err(format!("transaction reverted: {}", tx_hash))
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(TX_RECEIPT_POLL_INTERVAL).await;
        }
    }

    // ── Database helpers ──────────────────────────────────────────────────────

    async fn relay_already_recorded(&self, source_event_hash: &str) -> bool {
        sqlx::query(
            "SELECT 1 FROM treasury.relay_logs WHERE source_event_hash = $1",
        )
        .bind(source_event_hash)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
    }

    async fn insert_relay_log(
        &self,
        event: &HotPathEvent,
        dest_tx_hash: Option<&str>,
        status: &str,
    ) {
        let amount_str = event.amount.to_string();
        let recipient_str = format!("0x{}", bytes_to_hex_raw(event.recipient.as_slice()));
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO treasury.relay_logs
                (source_event_hash, dest_tx_hash, source_chain_id, dest_chain_id,
                 recipient, amount_wei, status)
            VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, $7)
            ON CONFLICT (source_event_hash) DO NOTHING
            "#,
        )
        .bind(&event.source_event_hash)
        .bind(dest_tx_hash)
        .bind(event.source_chain_id as i64)
        .bind(event.dest_chain_id as i64)
        .bind(&recipient_str)
        .bind(&amount_str)
        .bind(status)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "hot_path: failed to insert relay log");
        }
    }

    async fn update_relay_log(&self, source_event_hash: &str, dest_tx_hash: &str, status: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.relay_logs \
             SET dest_tx_hash = $1, status = $2, updated_at = now() \
             WHERE source_event_hash = $3",
        )
        .bind(dest_tx_hash)
        .bind(status)
        .bind(source_event_hash)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "hot_path: failed to update relay log");
        }
    }

    async fn update_relay_log_failed(&self, source_event_hash: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.relay_logs \
             SET status = 'failed', updated_at = now() \
             WHERE source_event_hash = $1",
        )
        .bind(source_event_hash)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "hot_path: failed to mark relay log as failed");
        }
    }

    // ── Event parsing ─────────────────────────────────────────────────────────

    fn parse_hot_path_event(&self, log: &RpcLog, source_chain_id: u64) -> Option<HotPathEvent> {
        // topics[0] = event selector
        // topics[1] = indexed sender   (address, left-padded to 32 bytes)
        // topics[2] = indexed recipient(address, left-padded to 32 bytes)
        // data      = abi_encode(uint256 amount, uint64 destChainId, bytes32 eventId)
        if log.topics.len() < 3 {
            return None;
        }

        let sender_raw = decode_hex(&log.topics[1])?;
        let recipient_raw = decode_hex(&log.topics[2])?;
        if sender_raw.len() < 32 || recipient_raw.len() < 32 {
            return None;
        }

        let sender = format!("0x{}", bytes_to_hex_raw(&sender_raw[12..32]));
        let recipient = Address::from_slice(&recipient_raw[12..32]);

        let data = decode_hex(&log.data)?;
        if data.len() < 96 {
            return None;
        }

        let amount_bytes: [u8; 32] = data[0..32].try_into().ok()?;
        let amount = U256::from_be_bytes(amount_bytes);
        // uint64 destChainId: right-aligned in its 32-byte slot (bytes 32..64).
        let dest_chain_id = u64::from_be_bytes(data[56..64].try_into().ok()?);
        let event_id = B256::from_slice(&data[64..96]);

        Some(HotPathEvent {
            source_chain_id,
            source_event_hash: log.transaction_hash.clone(),
            sender,
            recipient,
            amount,
            dest_chain_id,
            event_id,
        })
    }
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

/// ABI-encode `releaseHotPath(address,uint256,bytes32)` call data.
fn encode_release_hot_path(
    selector: &[u8; 4],
    recipient: &Address,
    amount: &U256,
    event_id: &B256,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(100);
    data.extend_from_slice(selector);
    data.extend_from_slice(&[0u8; 12]); // address left-padding
    data.extend_from_slice(recipient.as_slice());
    data.extend_from_slice(&amount.to_be_bytes::<32>());
    data.extend_from_slice(event_id.as_slice());
    data
}

/// Decode the `activeChainsCsv` string from an `ActivationPublished` event's
/// ABI-encoded data field and return the set of active chain IDs.
///
/// ABI head layout (6 params, 192 bytes total):
///   slot 0 → offset for `runId`
///   slot 1 → offset for `customerId`
///   slot 2 = `thresholdBps` (static uint256)
///   slot 3 → offset for `activeChainsCsv`
///   slot 4 → offset for `inactiveChainsCsv`
///   slot 5 = `timestamp` (static uint256)
fn decode_active_chains_from_event(hex_data: &str) -> Option<HashSet<u64>> {
    let data = decode_hex(hex_data)?;
    if data.len() < 6 * 32 {
        return None;
    }
    // Read the pointer at slot 3 (bytes 96..128).
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

// ── Minimal RLP encoding ──────────────────────────────────────────────────────

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

/// Build the RLP-encoded body shared between the signing payload and the
/// signed transaction (everything except v, r, s).
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
