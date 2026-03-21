//! gRPC server bootstrap.
//!
//! Initialises the database pool, constructs all service components, and
//! starts the tonic gRPC server on the configured `GRPC_PORT`.

use std::fs;
use std::sync::Arc;

use sqlx::PgPool;
use tonic::{Request, Response, Status};
use tracing::info;

use crate::hot_path::HotPath;
use crate::pool_manager::PoolManager;
use crate::proto::treasury::{
    health_check_response, treasury_service_server::TreasuryService, GetPoolDepthRequest,
    GetPoolDepthResponse, GetRelayStatusRequest, GetRelayStatusResponse, GetWatcherAlertsRequest,
    GetWatcherAlertsResponse, HealthCheckRequest, HealthCheckResponse,
};
use crate::watcher::Watcher;

pub struct TreasuryServer {
    pub pool: PgPool,
    pub hot_path: Arc<HotPath>,
    pub pool_manager: Arc<PoolManager>,
    pub watcher: Arc<Watcher>,
    /// Cached result of the startup relayer-key check.
    pub relayer_key_loaded: bool,
    /// Cached result of the startup RPC reachability check.
    pub rpc_reachable: bool,
}

#[tonic::async_trait]
impl TreasuryService for TreasuryServer {
    async fn health_check(
        &self,
        _req: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let db_connected = sqlx::query("SELECT 1").execute(&self.pool).await.is_ok();

        let status = if db_connected && self.rpc_reachable && self.relayer_key_loaded {
            health_check_response::Status::Serving
        } else {
            health_check_response::Status::NotServing
        };

        info!(
            db_connected,
            rpc_reachable = self.rpc_reachable,
            relayer_key_loaded = self.relayer_key_loaded,
            "health_check"
        );

        Ok(Response::new(HealthCheckResponse {
            status: status as i32,
            db_connected,
            rpc_reachable: self.rpc_reachable,
            relayer_key_loaded: self.relayer_key_loaded,
        }))
    }

    async fn get_relay_status(
        &self,
        req: Request<GetRelayStatusRequest>,
    ) -> Result<Response<GetRelayStatusResponse>, Status> {
        self.hot_path.get_relay_status(req).await
    }

    async fn get_pool_depth(
        &self,
        req: Request<GetPoolDepthRequest>,
    ) -> Result<Response<GetPoolDepthResponse>, Status> {
        self.pool_manager.get_pool_depth(req).await
    }

    async fn get_watcher_alerts(
        &self,
        req: Request<GetWatcherAlertsRequest>,
    ) -> Result<Response<GetWatcherAlertsResponse>, Status> {
        self.watcher.get_watcher_alerts(req).await
    }
}

// ── Startup checks ────────────────────────────────────────────────────────────

/// Check that the relayer key file exists and contains a valid 32-byte hex key.
pub fn check_relayer_key(path: &str) -> bool {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let hex = contents.trim().trim_start_matches("0x");
            hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
        }
        Err(_) => false,
    }
}

/// Fire an `eth_blockNumber` JSON-RPC call to verify reachability.
pub async fn check_rpc_reachable(
    rpc_urls: &std::collections::HashMap<u64, String>,
    http: &reqwest::Client,
) -> bool {
    if rpc_urls.is_empty() {
        return false;
    }
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });
    for url in rpc_urls.values() {
        match http.post(url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => continue,
            _ => return false,
        }
    }
    true
}
