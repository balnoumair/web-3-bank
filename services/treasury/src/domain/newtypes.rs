//! Newtype wrappers for treasury domain primitives.
//!
//! Prevent parameter mix-ups (e.g. swapping source_chain and dest_chain)
//! that raw `u64` / `String` types cannot catch at compile time.

/// A chain identifier (EVM chain ID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChainId(pub u64);

impl From<u64> for ChainId {
    fn from(v: u64) -> Self {
        ChainId(v)
    }
}
impl From<ChainId> for u64 {
    fn from(c: ChainId) -> Self {
        c.0
    }
}
impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A rebalance operation identifier (generated as "source-dest-timestamp_ms").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationId(pub String);

impl OperationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for OperationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::ops::Deref for OperationId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// A transaction hash (hex string, "0x…").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxHash(pub String);

impl TxHash {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for TxHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::ops::Deref for TxHash {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// A source event hash (relay idempotency key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHash(pub String);

impl EventHash {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Display for EventHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::ops::Deref for EventHash {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}
