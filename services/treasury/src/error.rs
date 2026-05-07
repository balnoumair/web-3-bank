//! Shared error types for Ethereum RPC and transaction operations.

use thiserror::Error;

/// Common error type for Ethereum RPC and transaction operations
/// shared across treasury modules.
#[derive(Debug, Error)]
pub enum TxError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("missing {field} in RPC response")]
    MissingField { field: &'static str },
    #[error("hex parse error: {0}")]
    HexParse(std::num::ParseIntError),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("transaction reverted: {tx_hash}")]
    TxReverted { tx_hash: String },
    #[error("timed out waiting for receipt: {tx_hash}")]
    TxTimeout { tx_hash: String },
    #[error("ECDSA signing failed: {0}")]
    Signing(String),
    #[error("invalid contract address")]
    InvalidAddress,
    #[error("signing key not loaded")]
    MissingKey,
    #[error("rebalance amount exceeds on-chain cap")]
    RebalanceCapExceeded,
    #[error("rebalance destination chain is not allowlisted")]
    DestChainNotAllowlisted,
    #[error("source pool depth is insufficient for rebalance")]
    PoolDepthInsufficient,
    #[error("failed after {attempts} attempts")]
    RetryExhausted { attempts: u32 },
}
