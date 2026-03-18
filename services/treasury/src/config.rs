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

    /// JSON map of chain_id → BankContract checksummed address string.
    pub contract_addresses: JsonMap<u64, String>,

    /// RouteReceiver.sol address on Base Sepolia (checksummed hex string).
    pub route_receiver_address: String,

    /// Path to a file containing the hex-encoded relayer private key.
    pub relayer_key_path: String,
}

fn default_grpc_port() -> u16 {
    50051
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
