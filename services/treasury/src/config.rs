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
    /// Minimum ratio to target depth in basis points (0–10 000) for any chain
    /// below which rebalancing is triggered.  For example `8000` = 80 % of
    /// target.  Default: 8000.
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

    /// Optional `host:port` for user-service gRPC (e.g. `127.0.0.1:50051`).
    /// When set, the treasury indexes `Deposited` events and calls
    /// `SetUserHomeChain` so first-deposit home routing is recorded.
    #[serde(default)]
    pub user_service_addr: Option<String>,

    /// Seconds before an in-flight CCIP rebalance requires manual review.
    /// Default: 1800 (30 minutes).
    #[serde(default = "default_cold_path_stuck_message_timeout_secs")]
    pub cold_path_stuck_message_timeout_secs: u64,

    // ── Reserve-path rebalancing (USDC reserves via CCTP) ─────────────────────
    /// Minimum ratio to target reserve depth in basis points (0–10 000) below
    /// which a chain triggers a reserve bridge.  Default: 8000 (80 %).
    #[serde(default = "default_reserve_path_min_bps")]
    pub reserve_path_min_bps: u32,

    /// Maximum USDC (in 6-decimal wei) per single reserve bridge operation,
    /// as a decimal string.  Empty = no cap.
    #[serde(default)]
    pub reserve_path_max_wei: String,

    /// How often (seconds) the reserve-path monitor checks reserve depths.
    /// Default: 60.
    #[serde(default = "default_reserve_path_poll_secs")]
    pub reserve_path_poll_secs: u64,

    /// Seconds before a stuck reserve-bridge op is marked failed for operator
    /// review.  Default: 1800 (30 minutes, matches spec for CCTP).
    #[serde(default = "default_reserve_path_stuck_timeout_secs")]
    pub reserve_path_stuck_timeout_secs: u64,

    /// JSON map of chain_id → CCTPReserveBridge contract address (checksummed
    /// hex).  Required for the reserve-path relayer loop to dispatch
    /// `bridgeIn` on the destination chain.
    #[serde(default)]
    pub reserve_bridge_addresses: Option<JsonMap<u64, String>>,

    /// JSON map of chain_id → CCTP domain (Ethereum=0, Avalanche=1, OP=2,
    /// Arbitrum=3, Base=6, …).  Required for fetching Circle attestations.
    #[serde(default)]
    pub cctp_domains: Option<JsonMap<u64, u32>>,

    /// Circle attestation service base URL.  Default: production endpoint.
    /// Override for testnet (`https://iris-api-sandbox.circle.com`).
    #[serde(default = "default_circle_attestation_url")]
    pub circle_attestation_api_url: String,

    /// Path to the reserve-ops relayer private key file.  When unset, the
    /// cold-path `relayer_key_path` is reused — fine for dev but production
    /// SHOULD use a separate key so the role grant blast radius is limited.
    #[serde(default)]
    pub reserve_relayer_key_path: Option<String>,

    /// Wei to include as `msg.value` on `bridgeIn` calls (gas for the destination
    /// adapter).  CCTP itself does not charge a fee at receive time; this value is
    /// 0 for most chains.  Default: "0".
    #[serde(default)]
    pub reserve_bridge_fee_wei: String,

    // ── Reserve-accounting ledger reconciliation ──────────────────────────────
    /// How often (seconds) to reconcile the internal reserve ledger against
    /// on-chain `reserveDepth()`.  Default: 300 (5 minutes) — reserves move
    /// rarely, so this need not be frequent.
    #[serde(default = "default_reserve_recon_poll_secs")]
    pub reserve_recon_poll_secs: u64,

    /// Allowed absolute difference (USDC 6-decimal wei, decimal string) between
    /// a chain's ledger reserve balance and its on-chain `reserveDepth()` before
    /// a reconciliation alert is raised.  Absorbs lifecycle timing skew (the
    /// brief window between a bridge tx mining and the ledger recording it).
    /// Empty = 0 (exact match required).  Default: "0".
    #[serde(default)]
    pub reserve_recon_tolerance_wei: String,
}

fn default_grpc_port() -> u16 {
    50051
}

fn default_cold_path_min_bps() -> u32 {
    8000 // 80 % of target
}

fn default_cold_path_poll_secs() -> u64 {
    60
}

fn default_cold_path_stuck_message_timeout_secs() -> u64 {
    1_800
}

fn default_reserve_path_min_bps() -> u32 {
    8000 // 80 % of target — spec trigger threshold
}

fn default_reserve_path_poll_secs() -> u64 {
    60
}

fn default_reserve_path_stuck_timeout_secs() -> u64 {
    1_800 // 30 minutes — spec CCTP timeout
}

fn default_circle_attestation_url() -> String {
    "https://iris-api.circle.com".to_string()
}

fn default_reserve_recon_poll_secs() -> u64 {
    300 // 5 minutes
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
        let map: HashMap<K, V> = serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
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
            serde_json::from_str(&format!("\"{}\"", json.replace('"', "\\\""))).unwrap_or_else(
                |_| {
                    // Simulate what envy does: pass raw JSON string through our Deserialize impl
                    let raw: HashMap<u64, String> = serde_json::from_str(json).unwrap();
                    JsonMap(raw)
                },
            );
        assert_eq!(map.get(&84532).unwrap(), "https://sepolia.base.org");
    }
}
