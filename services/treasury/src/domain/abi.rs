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

/// ABI-encode `bridgeReserve(uint64 destChainId, uint256 amount)`.
///
/// Shape is identical to `rebalance(uint64,uint256)` — kept as a separate helper for clarity
/// (different on-chain function, different selector).
pub fn encode_bridge_reserve(selector: &[u8; 4], dest_chain_id: u64, amount: &U256) -> Vec<u8> {
    let mut data = Vec::with_capacity(68);
    data.extend_from_slice(selector);
    data.extend_from_slice(&[0u8; 24]);
    data.extend_from_slice(&dest_chain_id.to_be_bytes());
    data.extend_from_slice(&amount.to_be_bytes::<32>());
    data
}

/// ABI-encode `bridgeIn(bytes message, bytes attestation)`.
///
/// ABI layout (two dynamic `bytes` args):
///   selector (4)
///   head[0]: offset to `message` tail   (= 0x40, since head is two 32-byte slots)
///   head[1]: offset to `attestation` tail (= 0x40 + 32 + padded(message.len()))
///   tail[0]: uint256(message.len) || message || zero-padding up to 32-byte multiple
///   tail[1]: uint256(attestation.len) || attestation || zero-padding
pub fn encode_bridge_in(selector: &[u8; 4], message: &[u8], attestation: &[u8]) -> Vec<u8> {
    fn pad_len(n: usize) -> usize {
        if n.is_multiple_of(32) { 0 } else { 32 - (n % 32) }
    }
    fn write_u256(buf: &mut Vec<u8>, val: u64) {
        buf.extend_from_slice(&[0u8; 24]);
        buf.extend_from_slice(&val.to_be_bytes());
    }

    let msg_pad = pad_len(message.len());
    let head_size = 64u64;
    let msg_offset = head_size; // = 64
    let att_offset = msg_offset + 32 + message.len() as u64 + msg_pad as u64;

    let total =
        4 + 64 + 32 + message.len() + msg_pad + 32 + attestation.len() + pad_len(attestation.len());
    let mut data = Vec::with_capacity(total);
    data.extend_from_slice(selector);
    write_u256(&mut data, msg_offset);
    write_u256(&mut data, att_offset);

    write_u256(&mut data, message.len() as u64);
    data.extend_from_slice(message);
    data.resize(data.len() + msg_pad, 0);

    write_u256(&mut data, attestation.len() as u64);
    data.extend_from_slice(attestation);
    data.resize(data.len() + pad_len(attestation.len()), 0);

    data
}

/// Extract the `messageId` from a `ReserveBridgeInitiated` event in receipt logs.
///
/// Bank emits `ReserveBridgeInitiated(bytes32 indexed messageId, uint64 indexed destChainId,
/// uint256 amount, bytes32 bridgeType)`. The messageId is `topics[1]` of the matching log.
pub fn extract_reserve_bridge_initiated_message_id(logs: &[serde_json::Value]) -> Option<String> {
    let event_topic = format!(
        "{}",
        alloy_primitives::keccak256(b"ReserveBridgeInitiated(bytes32,uint64,uint256,bytes32)")
    );
    for log in logs {
        if let Some(topics) = log["topics"].as_array() {
            let matches = topics
                .first()
                .and_then(|t| t.as_str())
                .map(|t| t.eq_ignore_ascii_case(&event_topic))
                .unwrap_or(false);
            if matches {
                if let Some(topic1) = topics.get(1).and_then(|t| t.as_str()) {
                    if topic1.len() == 66 {
                        return Some(topic1.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Best-effort extraction of a CCIP `messageId` (bytes32) from receipt logs.
///
/// CCIP's `CCIPSendRequested` event has the messageId as the second
/// topic. We look for the first log with at least 2 topics where the
/// second is a valid 32-byte hex value (0x-prefixed, 66 chars).
pub fn extract_ccip_message_id(logs: &[serde_json::Value]) -> Option<String> {
    let rebalance_initiated_topic = format!(
        "{}",
        alloy_primitives::keccak256(b"RebalanceInitiated(bytes32,uint64,uint256)")
    );

    for log in logs {
        if let Some(topics) = log["topics"].as_array() {
            let topic0_matches = topics
                .first()
                .and_then(|topic| topic.as_str())
                .map(|topic| topic.eq_ignore_ascii_case(&rebalance_initiated_topic))
                .unwrap_or(false);
            if topic0_matches {
                if let Some(topic1) = topics.get(1).and_then(|topic| topic.as_str()) {
                    if topic1.len() == 66 {
                        return Some(topic1.to_string());
                    }
                }
            }
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_bridge_reserve_layout() {
        let sel = [0xca, 0xfe, 0xba, 0xbe];
        let dest = 8453u64; // Base
        let amount = U256::from(123_456_789u64);
        let data = encode_bridge_reserve(&sel, dest, &amount);
        assert_eq!(data.len(), 68);
        assert_eq!(&data[..4], &sel);
        let enc_dest = u64::from_be_bytes(data[28..36].try_into().unwrap());
        assert_eq!(enc_dest, dest);
        let enc_amount = U256::from_be_bytes::<32>(data[36..68].try_into().unwrap());
        assert_eq!(enc_amount, amount);
    }

    #[test]
    fn encode_bridge_in_short_inputs() {
        let sel = [0xab, 0xcd, 0xef, 0x01];
        let message: Vec<u8> = (0..50u8).collect();
        let attestation = vec![0xAA; 65];
        let data = encode_bridge_in(&sel, &message, &attestation);

        // Selector
        assert_eq!(&data[..4], &sel);
        // head[0] = offset to message tail = 64
        let msg_offset = U256::from_be_bytes::<32>(data[4..36].try_into().unwrap());
        assert_eq!(msg_offset, U256::from(64u64));
        // head[1] = offset to attestation tail = 64 + 32 + padded(50) = 64 + 32 + 64 = 160
        let att_offset = U256::from_be_bytes::<32>(data[36..68].try_into().unwrap());
        assert_eq!(att_offset, U256::from(160u64));

        // message tail: length, then bytes, then padding to 32-byte boundary.
        // Body starts at byte 4 (selector) + head_offset (64) = 68 in the calldata.
        let msg_len = U256::from_be_bytes::<32>(data[68..100].try_into().unwrap());
        assert_eq!(msg_len, U256::from(50u64));
        assert_eq!(&data[100..150], &message[..]);
        // Bytes 150..164 should be padding (50 padded up to 64).
        assert!(data[150..164].iter().all(|&b| b == 0));

        // attestation tail starts at 4 + att_offset = 4 + 160 = 164.
        let att_len = U256::from_be_bytes::<32>(data[164..196].try_into().unwrap());
        assert_eq!(att_len, U256::from(65u64));
        assert_eq!(&data[196..261], &attestation[..]);
        // 65 padded up to 96 → 31 bytes of padding.
        assert!(data[261..292].iter().all(|&b| b == 0));
        assert_eq!(data.len(), 292);
    }

    #[test]
    fn encode_bridge_in_exact_32_byte_message_has_no_padding() {
        let sel = [0; 4];
        let message = vec![0x77; 32];
        let attestation = vec![0x88; 32];
        let data = encode_bridge_in(&sel, &message, &attestation);
        // Total: 4 + 64 (head) + 32 (msg.len) + 32 (msg) + 0 pad + 32 (att.len) + 32 (att) + 0 pad = 196
        assert_eq!(data.len(), 196);
        let att_offset = U256::from_be_bytes::<32>(data[36..68].try_into().unwrap());
        assert_eq!(att_offset, U256::from(128u64));
    }

    #[test]
    fn extracts_reserve_bridge_initiated_message_id() {
        let message_id = "0xabababababababababababababababababababababababababababababababab";
        let event_topic = format!(
            "{}",
            alloy_primitives::keccak256(b"ReserveBridgeInitiated(bytes32,uint64,uint256,bytes32)")
        );
        let dest_chain_topic =
            "0x0000000000000000000000000000000000000000000000000000000000002105";
        let logs = vec![
            serde_json::json!({"topics": ["0xdeadbeef"]}),
            serde_json::json!({"topics": [event_topic, message_id, dest_chain_topic]}),
        ];
        assert_eq!(
            extract_reserve_bridge_initiated_message_id(&logs),
            Some(message_id.to_string())
        );
    }

    #[test]
    fn extracts_reserve_bridge_initiated_returns_none_for_unrelated_logs() {
        let logs = vec![serde_json::json!({"topics": ["0xdeadbeef", "0x123"]})];
        assert_eq!(extract_reserve_bridge_initiated_message_id(&logs), None);
    }

    #[test]
    fn extracts_rebalance_initiated_message_id_before_other_topics() {
        let message_id = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let transfer_from_topic =
            "0x000000000000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let event_topic = format!(
            "{}",
            alloy_primitives::keccak256(b"RebalanceInitiated(bytes32,uint64,uint256)")
        );
        let logs = vec![
            serde_json::json!({"topics": [
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                transfer_from_topic
            ]}),
            serde_json::json!({"topics": [event_topic, message_id]}),
        ];

        assert_eq!(extract_ccip_message_id(&logs), Some(message_id.to_string()));
    }
}
