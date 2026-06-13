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
    #[serde(rename = "blockNumber", default)]
    pub block_number: String,
    #[serde(rename = "logIndex", default)]
    pub log_index: String,
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

/// Like [`fetch_logs`] but matches any of the given topic0 values (OR filter).
pub async fn fetch_logs_any(
    http: &reqwest::Client,
    rpc_url: &str,
    address: &str,
    topics: &[String],
    from_block: u64,
    to_block: u64,
) -> Vec<RpcLog> {
    if topics.is_empty() {
        return vec![];
    }
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getLogs",
        "params": [{
            "address": address,
            "topics": [topics],
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

/// Decode a hex quantity field from JSON-RPC (e.g. blockNumber, logIndex).
pub fn parse_hex_u64(hex: &str) -> Option<u64> {
    if hex.is_empty() {
        return None;
    }
    u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
}

/// Fetch the Unix timestamp for a block via `eth_getBlockByNumber`.
pub async fn fetch_block_timestamp(
    http: &reqwest::Client,
    rpc_url: &str,
    block_number: u64,
) -> Option<i64> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getBlockByNumber",
        "params": [format!("0x{:x}", block_number), false],
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
    let ts_hex = resp["result"]["timestamp"].as_str()?;
    let ts = u64::from_str_radix(ts_hex.trim_start_matches("0x"), 16).ok()?;
    Some(ts as i64)
}

/// Call a view function returning an address (32-byte ABI word, right-aligned).
pub async fn fetch_address_view(
    http: &reqwest::Client,
    rpc_url: &str,
    contract_addr: &str,
    selector: &[u8; 4],
) -> Option<String> {
    let call_data = format!("0x{}", super::encoding::bytes_to_hex(selector));
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": contract_addr, "data": call_data}, "latest"],
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
    let bytes = super::encoding::decode_hex(resp["result"].as_str()?)?;
    if bytes.len() < 32 {
        return None;
    }
    Some(format!("0x{}", super::encoding::bytes_to_hex(&bytes[12..32])))
}

/// Live SyncUSD `balanceOf(address)` via `eth_call`.
pub async fn fetch_balance_of(
    http: &reqwest::Client,
    rpc_url: &str,
    token_addr: &str,
    user_addr: &str,
    selector: &[u8; 4],
) -> Option<U256> {
    let call_data = super::encoding::encode_balance_of(selector, user_addr)?;
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": token_addr, "data": call_data}, "latest"],
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
    let bytes = super::encoding::decode_hex(resp["result"].as_str()?)?;
    if bytes.len() < 32 {
        return None;
    }
    let arr: [u8; 32] = bytes[..32].try_into().ok()?;
    Some(U256::from_be_bytes(arr))
}

pub async fn fetch_pool_depth(
    http: &reqwest::Client,
    rpc_url: &str,
    bank_addr: &str,
    selector: &[u8; 4],
) -> Option<U256> {
    fetch_u256_view(http, rpc_url, bank_addr, selector).await
}

pub async fn fetch_max_rebalance_amount(
    http: &reqwest::Client,
    rpc_url: &str,
    bank_addr: &str,
    selector: &[u8; 4],
) -> Option<U256> {
    fetch_u256_view(http, rpc_url, bank_addr, selector).await
}

pub async fn fetch_reserve_depth(
    http: &reqwest::Client,
    rpc_url: &str,
    bank_addr: &str,
    selector: &[u8; 4],
) -> Option<U256> {
    fetch_u256_view(http, rpc_url, bank_addr, selector).await
}

async fn fetch_u256_view(
    http: &reqwest::Client,
    rpc_url: &str,
    contract_addr: &str,
    selector: &[u8; 4],
) -> Option<U256> {
    let call_data = format!("0x{}", bytes_to_hex(selector));
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": contract_addr, "data": call_data}, "latest"],
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
    let resp: serde_json::Value = http.post(rpc_url).json(&body).send().await?.json().await?;
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
    let resp: serde_json::Value = http.post(rpc_url).json(&body).send().await?.json().await?;
    let hex = resp["result"]
        .as_str()
        .ok_or(TxError::MissingField { field: "gasPrice" })?;
    let gp = u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(TxError::HexParse)?;
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
    let resp: serde_json::Value = http.post(rpc_url).json(&body).send().await?.json().await?;
    if let Some(err) = resp.get("error") {
        return Err(TxError::Rpc(format!("eth_sendRawTransaction: {err}")));
    }
    resp["result"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or(TxError::MissingField { field: "tx hash" })
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
        let resp: serde_json::Value = http.post(rpc_url).json(&body).send().await?.json().await?;
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
        let resp: serde_json::Value = http.post(rpc_url).json(&body).send().await?.json().await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::U256;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_block_number_returns_parsed_value() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":"0x100"})),
            )
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let result = fetch_block_number(&http, &server.uri()).await;
        assert_eq!(result, Some(256));
    }

    #[tokio::test]
    async fn fetch_block_number_returns_none_on_error_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        assert!(fetch_block_number(&http, &server.uri()).await.is_none());
    }

    #[tokio::test]
    async fn fetch_pool_depth_returns_parsed_u256() {
        // 1_000_000 = 0xF4240, padded to 32 bytes big-endian
        let depth_hex = "0x00000000000000000000000000000000000000000000000000000000000f4240";
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result": depth_hex})),
            )
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let selector = [0x00u8; 4];
        let result = fetch_pool_depth(&http, &server.uri(), "0xBankContract", &selector).await;
        assert_eq!(result, Some(U256::from(1_000_000u64)));
    }

    #[tokio::test]
    async fn fetch_pool_depth_returns_none_on_short_result() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":"0x1234"})),
            )
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let selector = [0x00u8; 4];
        assert!(
            fetch_pool_depth(&http, &server.uri(), "0xBankContract", &selector)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn fetch_logs_returns_empty_vec_on_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let logs = fetch_logs(&http, &server.uri(), "0xAddr", "0xtopic", 0, 100).await;
        assert!(logs.is_empty());
    }

    #[tokio::test]
    async fn fetch_logs_returns_empty_result_array() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"jsonrpc":"2.0","id":1,"result":[]})),
            )
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let logs = fetch_logs(&http, &server.uri(), "0xAddr", "0xtopic", 0, 100).await;
        assert!(logs.is_empty());
    }
}
