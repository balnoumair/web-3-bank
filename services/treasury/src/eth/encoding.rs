//! Hex encoding/decoding, RLP encoding, and ABI event decoding utilities.

use std::collections::HashSet;

use alloy_primitives::U256;

// ── Hex encoding / decoding ──────────────────────────────────────────────────

/// Decode a hex string (with or without `0x` prefix) into bytes.
pub fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Encode a byte slice as lowercase hex (no `0x` prefix).
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

/// Encode `balanceOf(address)` calldata (selector + 32-byte padded address).
pub fn encode_balance_of(selector: &[u8; 4], address: &str) -> Option<String> {
    let addr_hex = address.strip_prefix("0x").unwrap_or(address);
    let addr_bytes = decode_hex(addr_hex)?;
    if addr_bytes.len() != 20 {
        return None;
    }
    let mut data = Vec::with_capacity(36);
    data.extend_from_slice(selector);
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(&addr_bytes);
    Some(format!("0x{}", bytes_to_hex(&data)))
}

// ── RLP encoding ─────────────────────────────────────────────────────────────

pub fn rlp_encode_uint(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![0x80]; // RLP encoding of zero is the empty string
    }
    let b = val.to_be_bytes();
    let start = b.iter().position(|&x| x != 0).unwrap_or(7);
    rlp_encode_bytes(&b[start..])
}

pub fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    let mut out = Vec::new();
    if data.len() <= 55 {
        out.push(0x80 + data.len() as u8);
    } else {
        let lb = data.len().to_be_bytes();
        let ls = lb.iter().position(|&x| x != 0).unwrap_or(7);
        let lm = &lb[ls..];
        out.push(0xb7 + lm.len() as u8);
        out.extend_from_slice(lm);
    }
    out.extend_from_slice(data);
    out
}

pub fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = items.iter().flat_map(|i| i.iter().copied()).collect();
    let mut out = Vec::new();
    if payload.len() <= 55 {
        out.push(0xc0 + payload.len() as u8);
    } else {
        let lb = payload.len().to_be_bytes();
        let ls = lb.iter().position(|&x| x != 0).unwrap_or(7);
        let lm = &lb[ls..];
        out.push(0xf7 + lm.len() as u8);
        out.extend_from_slice(lm);
    }
    out.extend_from_slice(&payload);
    out
}

/// Convert a U256 to its minimal big-endian byte representation.
/// Returns an empty vec for zero (RLP value 0 is encoded as empty bytes).
pub fn u256_to_trimmed_be(val: U256) -> Vec<u8> {
    if val.is_zero() {
        return vec![];
    }
    let bytes = val.to_be_bytes::<32>();
    let start = bytes.iter().position(|&x| x != 0).unwrap_or(31);
    bytes[start..].to_vec()
}

// ── Event decoding ───────────────────────────────────────────────────────────

/// Decode the `activeChainsCsv` string from an `ActivationPublished` event's
/// ABI-encoded data field and return the set of active chain IDs.
///
/// ABI head layout (6 params, 192 bytes total):
///   slot 0 → offset for `runId`
///   slot 1 → offset for `customerId`
///   slot 2 = `thresholdBps` (static uint256)
///   slot 3 → offset for `activeChainsCsv`
///   slot 4 → offset for `inactiveChainsCsv`
///   slot 5 = `timestamp` (static uint256)
pub fn decode_active_chains_from_event(hex_data: &str) -> Option<HashSet<u64>> {
    let data = decode_hex(hex_data)?;
    if data.len() < 6 * 32 {
        return None;
    }
    // Read the pointer at slot 3 (bytes 96..128).
    let ptr = u64::from_be_bytes(data[3 * 32 + 24..4 * 32].try_into().ok()?) as usize;
    if ptr + 32 > data.len() {
        return None;
    }
    let len = u64::from_be_bytes(data[ptr + 24..ptr + 32].try_into().ok()?) as usize;
    if ptr + 32 + len > data.len() {
        return None;
    }
    let csv = std::str::from_utf8(&data[ptr + 32..ptr + 32 + len]).ok()?;
    Some(
        csv.split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect(),
    )
}

/// Decode a uint64 stored as an indexed event topic (`bytes32` ABI word).
pub fn decode_indexed_u64_topic(topic: &str) -> Option<u64> {
    let data = decode_hex(topic)?;
    if data.len() != 32 {
        return None;
    }
    Some(u64::from_be_bytes(data[24..32].try_into().ok()?))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u256_trimmed_be_zero() {
        assert!(u256_to_trimmed_be(U256::ZERO).is_empty());
    }

    #[test]
    fn u256_trimmed_be_nonzero() {
        let v = U256::from(0x1234u64);
        let b = u256_to_trimmed_be(v);
        assert_eq!(b, vec![0x12, 0x34]);
    }

    #[test]
    fn hex_round_trip() {
        let original = vec![0xde, 0xad, 0xbe, 0xef];
        let hex = bytes_to_hex(&original);
        assert_eq!(hex, "deadbeef");
        assert_eq!(decode_hex(&hex), Some(original.clone()));
        assert_eq!(decode_hex(&format!("0x{hex}")), Some(original));
    }

    #[test]
    fn decode_hex_odd_length_returns_none() {
        assert_eq!(decode_hex("abc"), None);
    }

    #[test]
    fn decode_indexed_u64_topic_reads_low_word() {
        let topic = "0x0000000000000000000000000000000000000000000000000000000000014a34";
        assert_eq!(decode_indexed_u64_topic(topic), Some(84532));
    }
}
