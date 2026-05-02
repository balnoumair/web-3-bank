//! Pool manager module.
//!
//! Polls the `poolDepth()` view function on each Bank Contract at a regular
//! interval, records snapshots in `treasury.pool_snapshots`, and serves the
//! `GetPoolDepth` gRPC call from the latest snapshot.

use std::sync::Arc;
use std::time::Duration;

use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::config::Config;
use crate::domain::newtypes::ChainId;
use crate::domain::repository::PoolSnapshotRepository;
use crate::eth;
use crate::proto::treasury::{GetPoolDepthRequest, GetPoolDepthResponse};

const POLL_INTERVAL: Duration = Duration::from_secs(15);

pub struct PoolManager {
    snapshot_repo: Arc<dyn PoolSnapshotRepository>,
    config: Arc<Config>,
    http: reqwest::Client,
    /// 4-byte selector for `poolDepth()`
    pool_depth_selector: [u8; 4],
}

impl PoolManager {
    pub fn new(
        snapshot_repo: Arc<dyn PoolSnapshotRepository>,
        config: Arc<Config>,
        http: reqwest::Client,
    ) -> Arc<Self> {
        use alloy_primitives::keccak256;
        let hash = keccak256(b"poolDepth()");
        let mut sel = [0u8; 4];
        sel.copy_from_slice(&hash[..4]);

        Arc::new(Self {
            snapshot_repo,
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
        let chain_id = ChainId(req.into_inner().chain_id);
        match self.snapshot_repo.get_latest_depth(chain_id).await {
            Some(depth_wei) => Ok(Response::new(GetPoolDepthResponse { depth_wei })),
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
                match eth::fetch_pool_depth(
                    &self.http,
                    &rpc_url,
                    &bank_addr,
                    &self.pool_depth_selector,
                )
                .await
                {
                    Some(depth) => {
                        self.snapshot_repo
                            .record_snapshot(ChainId(chain_id), &depth)
                            .await;
                        info!(chain = chain_id, depth = %depth, "pool_manager: snapshot recorded");
                    }
                    None => {
                        warn!(chain = chain_id, "pool_manager: failed to fetch pool depth");
                    }
                }
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}
