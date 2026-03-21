//! Pure ABI encoding helpers for Bank Contract function calls.
//!
//! These are domain-level helpers because the ABI layout is defined by
//! the smart contract interface, which is part of the domain model.

use alloy_primitives::{Address, B256, U256};

/// ABI-encode `releaseHotPath(address to, uint256 amount, bytes32 sourceEventHash)`.
pub fn encode_release_hot_path(
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

/// ABI-encode `rebalance(uint64 destChainId, uint256 amount)`.
///
/// ABI head layout:
///   4 bytes  selector
///  32 bytes  destChainId  (uint64, right-aligned)
///  32 bytes  amount       (uint256, big-endian)
pub fn encode_rebalance(selector: &[u8; 4], dest_chain_id: u64, amount: &U256) -> Vec<u8> {
    let mut data = Vec::with_capacity(68);
    data.extend_from_slice(selector);
    // uint64 → 32-byte slot (zero-padded, right-aligned)
    data.extend_from_slice(&[0u8; 24]);
    data.extend_from_slice(&dest_chain_id.to_be_bytes());
    // uint256 → 32-byte slot (big-endian)
    data.extend_from_slice(&amount.to_be_bytes::<32>());
    data
}

/// Best-effort extraction of a CCIP `messageId` (bytes32) from receipt logs.
///
/// CCIP's `CCIPSendRequested` event has the messageId as the second
/// topic. We look for the first log with at least 2 topics where the
/// second is a valid 32-byte hex value (0x-prefixed, 66 chars).
pub fn extract_ccip_message_id(logs: &[serde_json::Value]) -> Option<String> {
    for log in logs {
        if let Some(topics) = log["topics"].as_array() {
            if topics.len() >= 2 {
                if let Some(topic1) = topics[1].as_str() {
                    if topic1.len() == 66 {
                        return Some(topic1.to_string());
                    }
                }
            }
        }
    }
    None
}
