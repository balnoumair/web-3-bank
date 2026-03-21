//! JSON-RPC client helpers for Ethereum node interaction.

use std::time::Duration;

use alloy_primitives::{Address, U256};

use super::encoding::{bytes_to_hex, decode_hex};
use crate::error::TxError;

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
