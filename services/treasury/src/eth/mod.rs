//! Shared Ethereum utilities: hex encoding, RLP, RPC helpers, EIP-1559 signing.
//!
//! Consolidates functions previously duplicated across hot_path, watcher,
//! cold_path, and pool_manager modules.

mod encoding;
mod rpc;
mod tx;

// Re-export everything so `use crate::eth::*` continues to work.
pub use encoding::*;
pub use rpc::*;
pub use tx::*;
