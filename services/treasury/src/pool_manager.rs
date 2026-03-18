//! Pool manager module.
//!
//! Polls the `poolDepth()` view function on each Bank Contract at a regular
//! interval, records snapshots in `treasury.pool_snapshots`, and serves the
//! `GetPoolDepth` gRPC call from the latest snapshot.

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::U256;
use sqlx::PgPool;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::config::Config;
use crate::proto::treasury::{GetPoolDepthRequest, GetPoolDepthResponse};

const POLL_INTERVAL: Duration = Duration::from_secs(15);

pub struct PoolManager {
    pool: PgPool,
    config: Arc<Config>,
    http: reqwest::Client,
    /// 4-byte selector for `poolDepth()`
    pool_depth_selector: [u8; 4],
}

impl PoolManager {
    pub fn new(pool: PgPool, config: Arc<Config>, http: reqwest::Client) -> Arc<Self> {
        use alloy_primitives::keccak256;
        let hash = keccak256(b"poolDepth()");
        let mut sel = [0u8; 4];
        sel.copy_from_slice(&hash[..4]);

        Arc::new(Self {
            pool,
            config,
            http,
            pool_depth_selector: sel,
        })
    }

    /// Spawn the background pool-depth polling task.
    pub fn spawn_background(self: Arc<Self>) {
        tokio::spawn(async move { self.poll_loop().await });
    }

    // ── gRPC handler ─────────────────────────────────────────────────────────

    pub async fn get_pool_depth(
        &self,
        req: Request<GetPoolDepthRequest>,
    ) -> Result<Response<GetPoolDepthResponse>, Status> {
        let chain_id = req.into_inner().chain_id as i64;

        let row = sqlx::query(
            "SELECT depth_wei::TEXT FROM treasury.pool_snapshots \
             WHERE chain_id = $1 \
             ORDER BY recorded_at DESC \
             LIMIT 1",
        )
        .bind(chain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        match row {
            Some(r) => {
                use sqlx::Row;
                let depth_wei: String = r.try_get("depth_wei").unwrap_or_else(|_| "0".into());
                Ok(Response::new(GetPoolDepthResponse { depth_wei }))
            }
            None => Err(Status::not_found("no pool depth snapshot for that chain")),
        }
    }

    // ── Background poll loop ──────────────────────────────────────────────────

    async fn poll_loop(self: Arc<Self>) {
        info!("pool_manager: depth polling started");
        loop {
            // Collect chain entries to avoid borrow-checker issues with the
            // async executor holding references across await points.
            let chains: Vec<(u64, String, String)> = self
                .config
                .rpc_urls
                .iter()
                .filter_map(|(&chain_id, rpc_url)| {
                    self.config
                        .contract_addresses
                        .get(&chain_id)
                        .map(|addr| (chain_id, rpc_url.clone(), addr.clone()))
                })
                .collect();

            for (chain_id, rpc_url, bank_addr) in chains {
                match self.fetch_pool_depth(&rpc_url, &bank_addr).await {
                    Some(depth) => {
                        self.record_snapshot(chain_id, &depth).await;
                    }
                    None => {
                        warn!(chain = chain_id, "pool_manager: failed to fetch pool depth");
                    }
                }
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    // ── RPC helper ────────────────────────────────────────────────────────────

    async fn fetch_pool_depth(&self, rpc_url: &str, bank_addr: &str) -> Option<U256> {
        let call_data = format!("0x{}", bytes_to_hex_raw(&self.pool_depth_selector));
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{"to": bank_addr, "data": call_data}, "latest"],
            "id": 1
        });
        let resp: serde_json::Value = self
            .http
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

    // ── Database helper ───────────────────────────────────────────────────────

    async fn record_snapshot(&self, chain_id: u64, depth: &U256) {
        let depth_str = depth.to_string();
        if let Err(e) = sqlx::query(
            "INSERT INTO treasury.pool_snapshots (chain_id, depth_wei) \
             VALUES ($1, $2::NUMERIC)",
        )
        .bind(chain_id as i64)
        .bind(&depth_str)
        .execute(&self.pool)
        .await
        {
            error!(chain = chain_id, err = %e, "pool_manager: failed to record snapshot");
        } else {
            info!(chain = chain_id, depth = %depth, "pool_manager: snapshot recorded");
        }
    }
}

// ── Helpers (duplicated from hot_path to keep modules self-contained) ────────

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn bytes_to_hex_raw(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}
