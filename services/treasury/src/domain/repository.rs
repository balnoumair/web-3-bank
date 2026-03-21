//! Repository traits (driven ports) for treasury persistence.
//!
//! These define the storage contracts that the domain layer requires.
//! Implementations live in the infrastructure layer (currently the
//! module structs' DB helper methods backed by sqlx/Postgres).

use alloy_primitives::U256;
use async_trait::async_trait;

use crate::domain::events::HotPathEvent;
use crate::domain::status::{AlertType, RelayStatus};

// ── Relay repository (hot-path) ─────────────────────────────────────────────

#[async_trait]
pub trait RelayRepository: Send + Sync {
    /// Check if a relay log already exists for this source event hash.
    async fn relay_already_recorded(&self, source_event_hash: &str) -> bool;

    /// Insert a new relay log entry.
    async fn insert_relay_log(
        &self,
        event: &HotPathEvent,
        dest_tx_hash: Option<&str>,
        status: RelayStatus,
    );

    /// Update a relay log entry with a destination tx hash and status.
    async fn update_relay_log(
        &self,
        source_event_hash: &str,
        dest_tx_hash: &str,
        status: RelayStatus,
    );

    /// Mark a relay log entry as failed.
    async fn update_relay_log_failed(&self, source_event_hash: &str);

    /// Get the relay status for a source event hash. Returns `None` if
    /// no relay log exists.
    async fn get_relay_status(&self, source_event_hash: &str) -> Option<String>;
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
    async fn op_in_flight(&self, source_chain: u64, dest_chain: u64) -> bool;

    /// Insert a new rebalance operation in `pending` status.
    async fn insert_rebalance_op(
        &self,
        op_id: &str,
        source_chain: u64,
        dest_chain: u64,
        amount: &U256,
    );

    /// Update a rebalance op to `submitted` with tx hash and optional CCIP ID.
    async fn update_rebalance_op_submitted(
        &self,
        op_id: &str,
        tx_hash: &str,
        ccip_message_id: Option<&str>,
    );

    /// Mark a rebalance op as `failed`.
    async fn update_rebalance_op_failed(&self, op_id: &str);
}

// ── Pool snapshot repository ────────────────────────────────────────────────

#[async_trait]
pub trait PoolSnapshotRepository: Send + Sync {
    /// Record a pool depth snapshot for a chain.
    async fn record_snapshot(&self, chain_id: u64, depth: &U256);

    /// Get the latest pool depth for a chain. Returns `None` if no
    /// snapshot exists.
    async fn get_latest_depth(&self, chain_id: i64) -> Option<String>;
}
