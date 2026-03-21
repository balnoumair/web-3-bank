//! Domain event types for treasury operations.
//!
//! These represent the on-chain events relevant to hot-path relay,
//! watcher verification, and cold-path rebalancing.

use alloy_primitives::{Address, B256, U256};

/// A `HotPathInitiated` event parsed from a source chain.
///
/// Used by the hot-path relayer to relay funds to the destination chain
/// via `releaseHotPath`.
#[derive(Debug, Clone)]
pub struct HotPathEvent {
    pub source_chain_id: u64,
    /// Transaction hash of the log — used as idempotency key in relay_logs.
    pub source_event_hash: String,
    pub sender: String,
    pub recipient: Address,
    pub amount: U256,
    pub dest_chain_id: u64,
    pub event_id: B256,
}

/// A cached `HotPathInitiated` event from a source chain (watcher view).
///
/// The watcher caches these to cross-reference against `HotPathReleased`
/// events for independent verification.
#[derive(Debug, Clone)]
pub struct InitiatedEvent {
    pub source_chain_id: u64,
    pub source_tx_hash: String,
    pub recipient: Address,
    pub amount: U256,
}

/// A `HotPathReleased` event observed on a destination chain.
///
/// The watcher verifies each release against the initiated cache to
/// detect mismatches or missing source events.
#[derive(Debug, Clone)]
pub struct ReleasedEvent {
    pub dest_chain_id: u64,
    pub dest_tx_hash: String,
    /// transferId == eventId from the corresponding HotPathInitiated.
    pub transfer_id: B256,
    pub recipient: Address,
    pub amount: U256,
}
