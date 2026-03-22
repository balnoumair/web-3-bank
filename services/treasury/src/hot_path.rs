//! Hot-path relay module.
//!
//! Listens for `HotPathInitiated` events on every source chain, validates the
//! destination chain is active (via `RouteReceiver.sol` `ActivationPublished`
//! events) and that the destination pool has sufficient depth, then submits
//! `releaseHotPath` on the destination chain and records the outcome in
//! `treasury.relay_logs`.
//!
//! # Bank Contract interface (Bank.sol)
//!
//! Event:
//!   `HotPathInitiated(address indexed sender, address indexed to,
//!                     uint256 amount, uint256 destinationChainId,
//!                     bytes32 eventHash, uint256 fee)`
//!
//! Write:
//!   `releaseHotPath(address to, uint256 amount, bytes32 sourceEventHash)`
//!
//! Read:
//!   `poolDepth() returns (uint256)`

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, Address, B256, U256};
use k256::ecdsa::SigningKey;
use tokio::sync::{Mutex, RwLock};
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::domain::abi::encode_release_hot_path;
use crate::domain::events::HotPathEvent;
use crate::domain::newtypes::{ChainId, EventHash, TxHash};
use crate::domain::relay::{evaluate_relay_eligibility, RelayDecision};
use crate::domain::repository::RelayRepository;
use crate::domain::status::RelayStatus;
use crate::error::TxError;
use crate::eth;
use crate::proto::treasury::{GetRelayStatusRequest, GetRelayStatusResponse};

// ── Constants ────────────────────────────────────────────────────────────────

const EVENT_POLL_INTERVAL: Duration = Duration::from_secs(2);
const ROUTE_RECEIVER_POLL_INTERVAL: Duration = Duration::from_secs(30);
const MAX_TX_WAIT: Duration = Duration::from_secs(60);
const MAX_RELAY_RETRIES: u32 = 3;
/// Maximum block range per `eth_getLogs` call (some RPCs cap this).
const MAX_BLOCK_RANGE: u64 = 2_000;

// ── Hot path module ──────────────────────────────────────────────────────────

pub struct HotPath {
    relay_repo: Arc<dyn RelayRepository>,
    config: Arc<Config>,
    http: reqwest::Client,
    /// Chain IDs currently in the active set per the latest ActivationPublished
    /// event. Seeded with all configured chains at startup so the relay works
    /// before the first CRE publish arrives.
    active_chains: Arc<RwLock<HashSet<ChainId>>>,
    /// Relayer ECDSA signing key (absent when the key file is missing/invalid).
    relayer_key: Option<Arc<SigningKey>>,
    /// Ethereum address derived from the relayer signing key.
    relayer_address: Option<Address>,
    /// Per-chain nonce cache to avoid an extra RPC round-trip on each tx.
    nonce_cache: Arc<Mutex<HashMap<ChainId, u64>>>,
    /// keccak256("HotPathInitiated(address,address,uint256,uint256,bytes32,uint256)")
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
    pub fn new(
        relay_repo: Arc<dyn RelayRepository>,
        config: Arc<Config>,
        http: reqwest::Client,
    ) -> Arc<Self> {
        let (relayer_key, relayer_address) = eth::load_signing_key(&config.relayer_key_path);

        // Seed active chains with every chain that has an RPC URL so the relay
        // can forward events before the first ActivationPublished arrives.
        let initial: HashSet<ChainId> = config.rpc_urls.keys().map(|&k| ChainId(k)).collect();

        let hot_path_topic =
            keccak256(b"HotPathInitiated(address,address,uint256,uint256,bytes32,uint256)");
        let activation_topic =
            keccak256(b"ActivationPublished(string,string,string,uint256,string,string,uint256)");

        let release_hash = keccak256(b"releaseHotPath(address,uint256,bytes32)");
        let pool_depth_hash = keccak256(b"poolDepth()");
        let mut release_selector = [0u8; 4];
        release_selector.copy_from_slice(&release_hash[..4]);
        let mut pool_depth_selector = [0u8; 4];
        pool_depth_selector.copy_from_slice(&pool_depth_hash[..4]);

        Arc::new(Self {
            relay_repo,
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
        let hash = EventHash(req.into_inner().source_event_hash);
        match self.relay_repo.get_relay_status(&hash).await {
            Some(status) => Ok(Response::new(GetRelayStatusResponse { status })),
            None => Err(Status::not_found("no relay log found for that event hash")),
        }
    }

    // ── Background loops ──────────────────────────────────────────────────────

    /// Poll every source chain for `HotPathInitiated` events and dispatch each
    /// to `relay_event`.
    async fn poll_events_loop(self: Arc<Self>) {
        let mut last_block: HashMap<ChainId, u64> = HashMap::new();
        info!("hot_path: event polling started");

        loop {
            // Collect chain entries to avoid borrow conflicts.
            let chains: Vec<(ChainId, String, String)> = self
                .config
                .rpc_urls
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
                let logs = eth::fetch_logs(
                    &self.http, &rpc_url, &bank_addr, &topic, scan_from, to_block,
                )
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
                    // decode_active_chains_from_event returns HashSet<u64>; convert to HashSet<ChainId>
                    let chains_converted: HashSet<ChainId> =
                        chains.into_iter().map(ChainId).collect();
                    info!(chains = ?chains_converted, "hot_path: activation state updated from RouteReceiver");
                    *self.active_chains.write().await = chains_converted;
                }
            }

            last_block = to_block + 1;
            tokio::time::sleep(ROUTE_RECEIVER_POLL_INTERVAL).await;
        }
    }

    // ── Core relay logic ──────────────────────────────────────────────────────

    async fn relay_event(&self, event: HotPathEvent) {
        // 1. Idempotency guard.
        if self
            .relay_repo
            .relay_already_recorded(&event.source_event_hash)
            .await
        {
            return;
        }

        // 2. Read destination chain active state.
        let chain_active = self
            .active_chains
            .read()
            .await
            .contains(&event.dest_chain_id);

        // 3. Resolve destination RPC and contract addresses from config.
        let dest_rpc = match self.config.rpc_urls.get(&event.dest_chain_id.0) {
            Some(u) => u.clone(),
            None => {
                warn!(
                    chain = event.dest_chain_id.0,
                    "hot_path: no RPC for dest chain"
                );
                return;
            }
        };
        let dest_bank = match self.config.contract_addresses.get(&event.dest_chain_id.0) {
            Some(a) => a.clone(),
            None => {
                warn!(
                    chain = event.dest_chain_id.0,
                    "hot_path: no bank contract for dest chain"
                );
                return;
            }
        };

        // 4. Fetch destination pool depth (skipped for inactive chains to avoid a
        //    pointless RPC call — the domain will reject on chain_active=false).
        let pool_depth = if chain_active {
            match eth::fetch_pool_depth(
                &self.http,
                &dest_rpc,
                &dest_bank,
                &self.pool_depth_selector,
            )
            .await
            {
                Some(d) => d,
                None => {
                    warn!(
                        src = %event.source_event_hash,
                        "hot_path: could not fetch pool depth — aborting relay"
                    );
                    return;
                }
            }
        } else {
            U256::ZERO // placeholder; evaluate_relay_eligibility short-circuits on inactive chain
        };

        // 5. Domain eligibility decision — pure, infra-free.
        match evaluate_relay_eligibility(chain_active, pool_depth, event.amount) {
            RelayDecision::Approved => {}
            RelayDecision::RejectedInactiveChain => {
                warn!(
                    src = %event.source_event_hash,
                    dest_chain = event.dest_chain_id.0,
                    "hot_path: rejected — destination chain not active"
                );
                self.relay_repo
                    .insert_relay_log(&event, None, RelayStatus::RejectedInactiveChain)
                    .await;
                return;
            }
            RelayDecision::RejectedInsufficientDepth => {
                warn!(
                    src = %event.source_event_hash,
                    dest_chain = event.dest_chain_id.0,
                    %event.amount,
                    %pool_depth,
                    "hot_path: rejected — insufficient pool depth"
                );
                self.relay_repo
                    .insert_relay_log(&event, None, RelayStatus::RejectedInsufficientDepth)
                    .await;
                return;
            }
        }

        // 6. Record pending.
        self.relay_repo
            .insert_relay_log(&event, None, RelayStatus::Pending)
            .await;

        // 7. Submit with retry.
        match self
            .submit_release_with_retry(&dest_rpc, &dest_bank, &event)
            .await
        {
            Ok(tx_hash) => {
                info!(
                    src = %event.source_event_hash,
                    dest_tx = %tx_hash,
                    "hot_path: relay completed"
                );
                self.relay_repo
                    .update_relay_log(&event.source_event_hash, &tx_hash, RelayStatus::Completed)
                    .await;
            }
            Err(e) => {
                error!(
                    src = %event.source_event_hash,
                    err = %e,
                    "hot_path: relay failed after retries"
                );
                self.relay_repo
                    .update_relay_log_failed(&event.source_event_hash)
                    .await;
            }
        }
    }

    async fn submit_release_with_retry(
        &self,
        rpc_url: &str,
        bank_addr: &str,
        event: &HotPathEvent,
    ) -> Result<TxHash, TxError> {
        let key = self.relayer_key.as_ref().ok_or(TxError::MissingKey)?;

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

        Err(TxError::RetryExhausted {
            attempts: MAX_RELAY_RETRIES,
        })
    }

    async fn submit_release_once(
        &self,
        rpc_url: &str,
        bank_addr: &str,
        event: &HotPathEvent,
        key: &SigningKey,
        chain_id: ChainId,
    ) -> Result<TxHash, TxError> {
        let relayer_addr = self.relayer_address.ok_or(TxError::MissingKey)?;

        let nonce = self.get_nonce(rpc_url, chain_id, &relayer_addr).await?;
        let (max_fee, max_priority_fee) = eth::fetch_gas_params(&self.http, rpc_url).await?;
        let gas_limit: u64 = 120_000; // conservative estimate for one storage write

        let call_data = encode_release_hot_path(
            &self.release_selector,
            &event.recipient,
            &event.amount,
            &event.event_id,
        );

        let bank_addr_bytes = eth::decode_hex(bank_addr)
            .filter(|b| b.len() == 20)
            .ok_or(TxError::InvalidAddress)?;

        let raw_hex = eth::sign_eip1559_tx(
            chain_id.0,
            nonce,
            max_priority_fee,
            max_fee,
            gas_limit,
            &bank_addr_bytes,
            &[],
            &call_data,
            key,
        )?;

        // Advance cached nonce before sending so concurrent calls don't reuse it.
        self.nonce_cache.lock().await.insert(chain_id, nonce + 1);

        let tx_hash_str = eth::send_raw_transaction(&self.http, rpc_url, &raw_hex).await?;
        eth::wait_for_receipt(&self.http, rpc_url, &tx_hash_str, MAX_TX_WAIT).await?;
        Ok(TxHash(tx_hash_str))
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

    fn parse_hot_path_event(
        &self,
        log: &eth::RpcLog,
        source_chain_id: ChainId,
    ) -> Option<HotPathEvent> {
        // topics[0] = event selector
        // topics[1] = indexed sender   (address, left-padded to 32 bytes)
        // topics[2] = indexed to       (address, left-padded to 32 bytes)
        // data      = abi_encode(uint256 amount, uint256 destinationChainId,
        //                        bytes32 eventHash, uint256 fee)
        if log.topics.len() < 3 {
            return None;
        }

        let sender_raw = eth::decode_hex(&log.topics[1])?;
        let recipient_raw = eth::decode_hex(&log.topics[2])?;
        if sender_raw.len() < 32 || recipient_raw.len() < 32 {
            return None;
        }

        let sender = format!("0x{}", eth::bytes_to_hex(&sender_raw[12..32]));
        let recipient = Address::from_slice(&recipient_raw[12..32]);

        let data = eth::decode_hex(&log.data)?;
        // 4 slots: amount (32) + destinationChainId (32) + eventHash (32) + fee (32)
        if data.len() < 128 {
            return None;
        }

        let amount_bytes: [u8; 32] = data[0..32].try_into().ok()?;
        let amount = U256::from_be_bytes(amount_bytes);
        // uint256 destinationChainId: right-aligned in its 32-byte slot (bytes 32..64).
        let dest_chain_id = ChainId(u64::from_be_bytes(data[56..64].try_into().ok()?));
        let event_id = B256::from_slice(&data[64..96]);
        // data[96..128] = fee (reserved, currently 0 — ignored)

        Some(HotPathEvent {
            source_chain_id,
            source_event_hash: EventHash(log.transaction_hash.clone()),
            sender,
            recipient,
            amount,
            dest_chain_id,
            event_id,
        })
    }
}
