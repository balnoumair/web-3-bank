//! Repository traits (driven ports) for treasury persistence.
//!
//! These define the storage contracts that the domain layer requires.
//! Implementations live in the infrastructure layer (currently the
//! module structs' DB helper methods backed by sqlx/Postgres).

use alloy_primitives::U256;
use async_trait::async_trait;

use crate::domain::events::HotPathEvent;
use crate::domain::newtypes::{ChainId, EventHash, OperationId, TxHash};
use crate::domain::status::{AlertType, RelayStatus};

// ── Shared types ─────────────────────────────────────────────────────────────

/// A relay log row returned by user-facing transfer queries.
pub struct TransferRow {
    pub source_event_hash: String,
    pub source_chain_id: i64,
    pub dest_chain_id: i64,
    pub sender: String,
    pub recipient: String,
    pub amount_wei: String,
    pub status: String,
    pub created_at: String,
}

// ── Relay repository (hot-path) ─────────────────────────────────────────────

#[async_trait]
pub trait RelayRepository: Send + Sync {
    /// Check if a relay log already exists for this source event hash.
    async fn relay_already_recorded(&self, source_event_hash: &EventHash) -> bool;

    /// Insert a new relay log entry.
    async fn insert_relay_log(
        &self,
        event: &HotPathEvent,
        dest_tx_hash: Option<&TxHash>,
        status: RelayStatus,
    );

    /// Update a relay log entry with a destination tx hash and status.
    async fn update_relay_log(
        &self,
        source_event_hash: &EventHash,
        dest_tx_hash: &TxHash,
        status: RelayStatus,
    );

    /// Mark a relay log entry as failed.
    async fn update_relay_log_failed(&self, source_event_hash: &EventHash);

    /// Get the relay status for a source event hash. Returns `None` if
    /// no relay log exists.
    async fn get_relay_status(&self, source_event_hash: &EventHash) -> Option<String>;

    /// Sum of `amount_wei` for all completed relays to this recipient address.
    /// Returns `"0"` if none found.
    async fn get_balance(&self, address: &str) -> String;

    /// Most recent relay log rows for a recipient address, newest first.
    async fn get_recent_transfers(&self, address: &str, limit: i64) -> Vec<TransferRow>;
}

// ── Watcher repository ──────────────────────────────────────────────────────

#[async_trait]
pub trait WatcherRepository: Send + Sync {
    /// Returns true if a watcher alert already exists for this transfer ID.
    async fn already_verified(&self, transfer_id_hex: &str) -> bool;

    /// Insert a verification alert.
    async fn insert_alert(&self, transfer_id_hex: &str, alert_type: AlertType, detail: &str);

    /// Get the most recent alert IDs, up to `limit`.
    async fn get_alert_ids(&self, limit: i64) -> Result<Vec<String>, String>;
}

// ── Rebalance repository (cold-path) ────────────────────────────────────────

#[async_trait]
pub trait RebalanceRepository: Send + Sync {
    /// Returns true if a pending or submitted op for this chain pair
    /// was created within the last 24 hours.
    async fn op_in_flight(&self, source_chain: ChainId, dest_chain: ChainId) -> bool;

    /// Insert a new rebalance operation in `pending` status.
    async fn insert_rebalance_op(
        &self,
        op_id: &OperationId,
        source_chain: ChainId,
        dest_chain: ChainId,
        amount: &U256,
    );

    /// Update a rebalance op to `submitted` with tx hash and optional CCIP ID.
    async fn update_rebalance_op_submitted(
        &self,
        op_id: &OperationId,
        tx_hash: &TxHash,
        ccip_message_id: Option<&str>,
    );

    /// Mark a rebalance op as `failed`.
    async fn update_rebalance_op_failed(&self, op_id: &OperationId);
}

// ── Reserve repository (USDC reserve rebalance) ─────────────────────────────

/// One in-flight reserve bridge operation, used by `reserve_path` when driving the relayer and
/// watcher loops. Mirrors a row of `treasury.reserve_ops`.
#[derive(Debug, Clone)]
pub struct ReserveOpRow {
    pub op_id: String,
    pub source_chain_id: i64,
    pub dest_chain_id: i64,
    pub amount_wei: String,
    pub bridge_type: String,
    pub bridge_message_id: Option<String>,
    pub source_tx_hash: Option<String>,
    pub cctp_message_bytes: Option<String>,
    pub cctp_attestation: Option<String>,
    pub dest_tx_hash: Option<String>,
    pub status: String,
}

#[async_trait]
pub trait ReserveRepository: Send + Sync {
    /// True iff a recent (24h) `pending`/`submitted`/`relayed` op exists for this chain pair.
    async fn op_in_flight(&self, source_chain: ChainId, dest_chain: ChainId) -> bool;

    /// Insert a new reserve op in `pending` status.
    async fn insert_reserve_op(
        &self,
        op_id: &OperationId,
        source_chain: ChainId,
        dest_chain: ChainId,
        amount: &U256,
        bridge_type: &str,
    );

    /// Mark a reserve op as `submitted`, attaching the source tx hash and the `messageId`
    /// emitted by `ReserveBridgeInitiated`.
    async fn update_submitted(
        &self,
        op_id: &OperationId,
        source_tx_hash: &TxHash,
        bridge_message_id: Option<&str>,
    );

    /// Persist Circle's `(message, attestation)` once fetched. Status is unchanged here —
    /// the relayer loop reads these columns to decide when to dispatch `bridgeIn`.
    async fn update_attestation(
        &self,
        op_id: &OperationId,
        cctp_message_bytes: &str,
        cctp_attestation: &str,
    );

    /// Mark a reserve op as `relayed` and attach the destination `bridgeIn` tx hash.
    async fn update_relayed(&self, op_id: &OperationId, dest_tx_hash: &TxHash);

    /// Mark a reserve op as `completed` (destination `ReserveBridgeCompleted` event observed).
    async fn update_completed(&self, op_id: &OperationId);

    /// Mark a reserve op as `failed` (timeout or unrecoverable error).
    async fn update_failed(&self, op_id: &OperationId);

    /// Fetch ops in `submitted` status (awaiting Circle attestation) for relayer loop.
    async fn list_awaiting_attestation(&self) -> Vec<ReserveOpRow>;

    /// Fetch ops in `relayed` status (awaiting on-chain completion) for watcher loop.
    async fn list_awaiting_completion(&self) -> Vec<ReserveOpRow>;

    /// Fetch a single op by its bridge_message_id and destination chain. Returns `None` if no
    /// match — used by the watcher to correlate observed `ReserveBridgeCompleted` events.
    async fn find_by_dest_and_message_id(
        &self,
        dest_chain: ChainId,
        bridge_message_id: &str,
    ) -> Option<ReserveOpRow>;

    /// Fetch in-flight ops (any pre-`completed` status) older than `stuck_after_secs` so the
    /// orchestrator can fail them and alert.
    async fn list_stuck(&self, stuck_after_secs: i64) -> Vec<ReserveOpRow>;
}

// ── Reserve ledger repository (internal double-entry mirror) ────────────────

/// Read + bootstrap surface for the internal reserve-accounting ledger.
///
/// The lifecycle transfers (initiation / completion / reversal) are written
/// atomically with the `reserve_ops` status change inside the reserve adapter's
/// transaction — see `db::reserve_ledger_repo::record_transfer_tx`. This port
/// therefore exposes only what the bootstrap and reconciliation loops need:
/// reading derived balances and seeding opening balances. The ledger is a
/// secondary mirror, never the source of truth.
#[async_trait]
pub trait ReserveLedgerRepository: Send + Sync {
    /// Current ledger balance of a chain's reserve account (credits − debits),
    /// saturating at zero. Compared against on-chain `reserveDepth()` during
    /// reconciliation.
    async fn account_balance(&self, chain: ChainId) -> U256;

    /// Current balance of the shared in-transit account: total value mid-bridge
    /// right now. This is the question the chain cannot answer on its own.
    async fn in_transit_balance(&self) -> U256;

    /// Seed a chain's reserve account from genesis with its current on-chain
    /// depth, so the ledger starts reconciled. Idempotent per chain (keyed
    /// `opening:<chain>`): safe to call on every startup.
    async fn seed_opening_balance(&self, chain: ChainId, depth: U256);

    /// Whether an opening balance has already been seeded for this chain.
    async fn has_opening_balance(&self, chain: ChainId) -> bool;
}

// ── Decommission repository ─────────────────────────────────────────────────

/// Audit row status for `treasury.decommission_ops`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecommissionOpStatus {
    Pending,
    Submitted,
    Completed,
    Paused,
    Failed,
}

impl DecommissionOpStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Submitted => "submitted",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::Failed => "failed",
        }
    }
}

#[async_trait]
pub trait DecommissionRepository: Send + Sync {
    async fn completed_holders(&self, source_chain: ChainId, target_chain: ChainId) -> Vec<String>;

    async fn insert_holder_op(
        &self,
        op_id: &OperationId,
        source_chain: ChainId,
        target_chain: ChainId,
        holder_address: &str,
        amount: &U256,
        status: DecommissionOpStatus,
    );

    async fn mark_holder_submitted(
        &self,
        op_id: &OperationId,
        src_message_id: Option<&str>,
        dst_tx_hash: Option<&TxHash>,
    );

    async fn mark_holder_completed(&self, op_id: &OperationId);

    async fn mark_op_failed(&self, op_id: &OperationId, failure_reason: &str);
}

// ── Pool snapshot repository ────────────────────────────────────────────────

#[async_trait]
pub trait PoolSnapshotRepository: Send + Sync {
    /// Record a pool depth snapshot for a chain.
    async fn record_snapshot(&self, chain_id: ChainId, depth: &U256);

    /// Get the latest pool depth for a chain. Returns `None` if no
    /// snapshot exists.
    async fn get_latest_depth(&self, chain_id: ChainId) -> Option<String>;
}
