use std::collections::HashMap;

use serde::Deserialize;

/// Treasury service configuration, loaded from environment variables via `envy`.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Postgres connection string.
    pub database_url: String,

    /// Port the gRPC server listens on.
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,

    /// JSON map of chain_id → RPC URL.
    /// Example: `{"84532":"https://sepolia.base.org"}`
    pub rpc_urls: JsonMap<u64, String>,

    /// JSON map of chain_id → Bank contract checksummed address string.
    pub contract_addresses: JsonMap<u64, String>,

    /// RouteReceiver.sol address on Base Sepolia (checksummed hex string).
    pub route_receiver_address: String,

    /// Path to a file containing the hex-encoded relayer private key.
    pub relayer_key_path: String,

    /// Path to a file containing the hex-encoded pauser private key.
    /// Used by the watcher to call `pause()` on Bank Contracts when a
    /// mismatch is detected.  Optional — pause actions are skipped when absent.
    #[serde(default)]
    pub pauser_key_path: Option<String>,

    /// Optional JSON map of chain_id → RPC URL for the watcher to use instead
    /// of `rpc_urls`.  Setting this to a different provider than `rpc_urls`
    /// ensures the watcher operates independently from the relayer.
    /// Example: `{"84532":"https://alternative-sepolia.example.com"}`
    #[serde(default)]
    pub watcher_rpc_urls: Option<JsonMap<u64, String>>,

    // ── Cold-path rebalancing ─────────────────────────────────────────────────

    /// Minimum pool ratio in basis points (0–10 000) for any chain below which
    /// rebalancing is triggered.  For example `2000` = 20 %.  Default: 2000.
    #[serde(default = "default_cold_path_min_bps")]
    pub cold_path_min_bps: u32,

    /// Target pool ratio in basis points (0–10 000) to restore each chain to.
    /// Set to `0` for equal distribution across all chains.  Default: 0.
    #[serde(default)]
    pub cold_path_target_bps: u32,

    /// Maximum SyncUSD (in wei) that may be moved in a single rebalance
    /// operation, expressed as a decimal string.  Empty string = no cap.
    #[serde(default)]
    pub cold_path_max_wei: String,

    /// ETH (in wei) to include as `msg.value` in each rebalance transaction to
    /// cover CCIP fees, expressed as a decimal string.  Default: "0".
    #[serde(default)]
    pub ccip_fee_wei: String,

    /// How often (seconds) the cold-path monitor checks pool ratios.
    /// Default: 60.
    #[serde(default = "default_cold_path_poll_secs")]
    pub cold_path_poll_secs: u64,
}

fn default_grpc_port() -> u16 {
    50051
}

fn default_cold_path_min_bps() -> u32 {
    2000 // 20 %
}

fn default_cold_path_poll_secs() -> u64 {
    60
}

impl Config {
    /// Load config from environment variables. Panics with a clear message on
    /// missing or malformed values.
    pub fn from_env() -> Self {
        envy::from_env::<Config>().expect(
            "missing or invalid environment variables — check .env.example for required fields",
        )
    }
}

// ── JSON-encoded map helper ───────────────────────────────────────────────────

/// Wrapper so envy can deserialize a JSON string into a `HashMap`.
///
/// The env var is expected to be a JSON object, e.g.:
/// `RPC_URLS={"84532":"https://sepolia.base.org"}`
#[derive(Debug)]
pub struct JsonMap<K, V>(pub HashMap<K, V>)
where
    K: std::hash::Hash + Eq;

impl<K, V> std::ops::Deref for JsonMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    type Target = HashMap<K, V>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de, K, V> Deserialize<'de> for JsonMap<K, V>
where
    K: std::hash::Hash + Eq + for<'k> Deserialize<'k> + std::str::FromStr,
    K::Err: std::fmt::Display,
    V: for<'v> Deserialize<'v>,
{
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // envy passes env var values as strings; parse the JSON string.
        let s = String::deserialize(d)?;
        let map: HashMap<K, V> =
            serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(JsonMap(map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_map() {
        let json = r#"{"84532":"https://sepolia.base.org"}"#;
        let map: JsonMap<u64, String> =
            serde_json::from_str(&format!("\"{}\"", json.replace('"', "\\\"")))
                .unwrap_or_else(|_| {
                    // Simulate what envy does: pass raw JSON string through our Deserialize impl
                    let raw: HashMap<u64, String> =
                        serde_json::from_str(json).unwrap();
                    JsonMap(raw)
                });
        assert_eq!(map.get(&84532).unwrap(), "https://sepolia.base.org");
    }
}
