//! Shared Ethereum utilities: hex encoding, RLP, RPC helpers, EIP-1559 signing.
//!
//! Consolidates functions previously duplicated across hot_path, watcher,
//! cold_path, and pool_manager modules.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, Address, U256};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, Signature, SigningKey};

use crate::error::TxError;

// ── Hex encoding / decoding ──────────────────────────────────────────────────

/// Decode a hex string (with or without `0x` prefix) into bytes.
pub fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() % 2 != 0 {
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

// ── Signing key ──────────────────────────────────────────────────────────────

/// Load a signing key from a hex-encoded file and derive its Ethereum address.
pub fn load_signing_key(path: &str) -> (Option<Arc<SigningKey>>, Option<Address>) {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let hex = contents.trim().trim_start_matches("0x");
    let bytes = match decode_hex(hex) {
        Some(b) if b.len() == 32 => b,
        _ => return (None, None),
    };
    let key = match SigningKey::from_bytes(bytes.as_slice().into()) {
        Ok(k) => k,
        Err(_) => return (None, None),
    };
    let uncompressed = key.verifying_key().to_encoded_point(false);
    let hash = keccak256(&uncompressed.as_bytes()[1..]); // skip 0x04 prefix
    let addr = Address::from_slice(&hash[12..]);
    (Some(Arc::new(key)), Some(addr))
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

// ── EIP-1559 transaction building & signing ──────────────────────────────────

/// Build the RLP-encoded body shared between the signing payload and the
/// signed transaction (everything except v, r, s).
#[allow(clippy::too_many_arguments)]
fn build_tx_rlp(
    chain_id: u64,
    nonce: u64,
    max_priority_fee: u64,
    max_fee: u64,
    gas_limit: u64,
    to: &[u8],
    value: &[u8],
    data: &[u8],
) -> Vec<u8> {
    rlp_encode_list(&[
        rlp_encode_uint(chain_id),
        rlp_encode_uint(nonce),
        rlp_encode_uint(max_priority_fee),
        rlp_encode_uint(max_fee),
        rlp_encode_uint(gas_limit),
        rlp_encode_bytes(to),
        rlp_encode_bytes(value),
        rlp_encode_bytes(data),
        rlp_encode_list(&[]), // empty access list
    ])
}

/// Sign an EIP-1559 transaction and return the raw hex-encoded bytes (with `0x` prefix).
///
/// Pass `&[]` for `value` when sending 0 ETH.
#[allow(clippy::too_many_arguments)]
pub fn sign_eip1559_tx(
    chain_id: u64,
    nonce: u64,
    max_priority_fee: u64,
    max_fee: u64,
    gas_limit: u64,
    to: &[u8],
    value: &[u8],
    call_data: &[u8],
    key: &SigningKey,
) -> Result<String, TxError> {
    // 1. Build signing payload: 0x02 || rlp([chain_id, nonce, ...])
    let signing_rlp = build_tx_rlp(
        chain_id,
        nonce,
        max_priority_fee,
        max_fee,
        gas_limit,
        to,
        value,
        call_data,
    );
    let mut to_sign = vec![0x02u8];
    to_sign.extend_from_slice(&signing_rlp);
    let hash = keccak256(&to_sign);

    // 2. ECDSA sign
    let (sig, recid): (Signature, RecoveryId) = key
        .sign_prehash(hash.as_slice())
        .map_err(|e| TxError::Signing(e.to_string()))?;

    let r_bytes = sig.r().to_bytes();
    let s_bytes = sig.s().to_bytes();
    let v = recid.to_byte() as u64;

    // 3. Build final signed transaction: 0x02 || rlp([..., v, r, s])
    let signed_rlp = rlp_encode_list(&[
        rlp_encode_uint(chain_id),
        rlp_encode_uint(nonce),
        rlp_encode_uint(max_priority_fee),
        rlp_encode_uint(max_fee),
        rlp_encode_uint(gas_limit),
        rlp_encode_bytes(to),
        rlp_encode_bytes(value),
        rlp_encode_bytes(call_data),
        rlp_encode_list(&[]), // empty access list
        rlp_encode_uint(v),
        rlp_encode_bytes(&r_bytes),
        rlp_encode_bytes(&s_bytes),
    ]);
    let mut raw_tx = vec![0x02u8];
    raw_tx.extend_from_slice(&signed_rlp);
    Ok(format!("0x{}", bytes_to_hex(&raw_tx)))
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

// ── JSON-RPC types ───────────────────────────────────────────────────────────

/// A log entry from `eth_getLogs` JSON-RPC responses.
#[derive(Debug, serde::Deserialize)]
pub struct RpcLog {
    #[serde(rename = "transactionHash", default)]
    pub transaction_hash: String,
    #[serde(default)]
    pub topics: Vec<String>,
    pub data: String,
}

// ── JSON-RPC helpers ─────────────────────────────────────────────────────────

pub async fn fetch_block_number(http: &reqwest::Client, rpc_url: &str) -> Option<u64> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "method": "eth_blockNumber", "params": [], "id": 1
    });
    let resp: serde_json::Value = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let hex = resp["result"].as_str()?;
    u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
}

pub async fn fetch_logs(
    http: &reqwest::Client,
    rpc_url: &str,
    address: &str,
    topic: &str,
    from_block: u64,
    to_block: u64,
) -> Vec<RpcLog> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getLogs",
        "params": [{
            "address": address,
            "topics": [topic],
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock":   format!("0x{:x}", to_block)
        }],
        "id": 1
    });
    let resp: serde_json::Value = match http.post(rpc_url).json(&body).send().await {
        Ok(r) => match r.json().await {
            Ok(v) => v,
            Err(_) => return vec![],
        },
        Err(_) => return vec![],
    };
    serde_json::from_value(resp["result"].clone()).unwrap_or_default()
}

pub async fn fetch_pool_depth(
    http: &reqwest::Client,
    rpc_url: &str,
    bank_addr: &str,
    selector: &[u8; 4],
) -> Option<U256> {
    let call_data = format!("0x{}", bytes_to_hex(selector));
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": bank_addr, "data": call_data}, "latest"],
        "id": 1
    });
    let resp: serde_json::Value = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let bytes = decode_hex(resp["result"].as_str()?)?;
    if bytes.len() < 32 {
        return None;
    }
    let arr: [u8; 32] = bytes[..32].try_into().ok()?;
    Some(U256::from_be_bytes(arr))
}

/// Fetch the pending nonce for an address via `eth_getTransactionCount`.
pub async fn fetch_nonce(
    http: &reqwest::Client,
    rpc_url: &str,
    addr: &Address,
) -> Result<u64, TxError> {
    let addr_hex = format!("0x{}", bytes_to_hex(addr.as_slice()));
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionCount",
        "params": [addr_hex, "pending"],
        "id": 1
    });
    let resp: serde_json::Value = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let hex = resp["result"]
        .as_str()
        .ok_or(TxError::MissingField { field: "nonce" })?;
    u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(TxError::HexParse)
}

pub async fn fetch_gas_params(
    http: &reqwest::Client,
    rpc_url: &str,
) -> Result<(u64, u64), TxError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "method": "eth_gasPrice", "params": [], "id": 1
    });
    let resp: serde_json::Value = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    let hex = resp["result"]
        .as_str()
        .ok_or(TxError::MissingField { field: "gasPrice" })?;
    let gp =
        u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(TxError::HexParse)?;
    let tip = gp / 10;
    Ok((gp + tip, tip)) // (maxFeePerGas, maxPriorityFeePerGas)
}

pub async fn send_raw_transaction(
    http: &reqwest::Client,
    rpc_url: &str,
    raw_hex: &str,
) -> Result<String, TxError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_sendRawTransaction",
        "params": [raw_hex],
        "id": 1
    });
    let resp: serde_json::Value = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    if let Some(err) = resp.get("error") {
        return Err(TxError::Rpc(format!("eth_sendRawTransaction: {err}")));
    }
    resp["result"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or(TxError::MissingField {
            field: "tx hash",
        })
}

/// Wait for a transaction receipt, returning `Ok(())` on success (status 0x1).
pub async fn wait_for_receipt(
    http: &reqwest::Client,
    rpc_url: &str,
    tx_hash: &str,
    timeout: Duration,
) -> Result<(), TxError> {
    let deadline = tokio::time::Instant::now() + timeout;
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
        "id": 1
    });
    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(TxError::TxTimeout {
                tx_hash: tx_hash.to_string(),
            });
        }
        let resp: serde_json::Value = http
            .post(rpc_url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        if let Some(receipt) = resp["result"].as_object() {
            match receipt.get("status").and_then(|s| s.as_str()) {
                Some("0x1") => return Ok(()),
                Some("0x0") => {
                    return Err(TxError::TxReverted {
                        tx_hash: tx_hash.to_string(),
                    })
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Wait for a transaction receipt and return the raw log entries from it.
pub async fn wait_for_receipt_logs(
    http: &reqwest::Client,
    rpc_url: &str,
    tx_hash: &str,
    timeout: Duration,
) -> Result<Vec<serde_json::Value>, TxError> {
    let deadline = tokio::time::Instant::now() + timeout;
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
        "id": 1
    });
    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(TxError::TxTimeout {
                tx_hash: tx_hash.to_string(),
            });
        }
        let resp: serde_json::Value = http
            .post(rpc_url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        if let Some(receipt) = resp["result"].as_object() {
            match receipt.get("status").and_then(|s| s.as_str()) {
                Some("0x1") => {
                    let logs = receipt
                        .get("logs")
                        .and_then(|l| l.as_array())
                        .cloned()
                        .unwrap_or_default();
                    return Ok(logs);
                }
                Some("0x0") => {
                    return Err(TxError::TxReverted {
                        tx_hash: tx_hash.to_string(),
                    })
                }
                _ => {}
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
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
}
