//! Strongly-typed status enums for treasury operations.
//!
//! Replaces raw `&str` status literals with compile-time checked enums.

use std::fmt;

/// Status of a hot-path relay operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayStatus {
    Pending,
    Completed,
    Failed,
    RejectedInactiveChain,
    RejectedInsufficientDepth,
}

impl RelayStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::RejectedInactiveChain => "rejected_inactive_chain",
            Self::RejectedInsufficientDepth => "rejected_insufficient_depth",
        }
    }
}

impl fmt::Display for RelayStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Status of a cold-path rebalance operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebalanceStatus {
    Pending,
    Submitted,
    Failed,
}

impl RebalanceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Submitted => "submitted",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for RebalanceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classification of a watcher verification alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertType {
    Verified,
    Mismatch,
    SourceNotFound,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verified => "Verified",
            Self::Mismatch => "Mismatch",
            Self::SourceNotFound => "SourceNotFound",
        }
    }
}

impl fmt::Display for AlertType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
