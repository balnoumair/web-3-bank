mod cold_path;
mod config;
mod db;
pub mod domain;
mod error;
mod eth;
mod hot_path;
mod pool_manager;
mod server;
mod watcher;

/// Generated gRPC types and server traits from proto/treasury.proto.
pub mod proto {
    pub mod treasury {
        tonic::include_proto!("treasury");
    }
}

use std::net::SocketAddr;
use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use tonic::transport::Server;
use tracing::info;

use crate::cold_path::ColdPath;
use crate::config::Config;
use crate::hot_path::HotPath;
use crate::pool_manager::PoolManager;
use crate::proto::treasury::treasury_service_server::TreasuryServiceServer;
use crate::server::{check_relayer_key, check_rpc_reachable, TreasuryServer};
use crate::watcher::Watcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "treasury=info".into()),
        )
        .init();

    let cfg = Arc::new(Config::from_env());

    // ── 1. Database: run migrations + verify connectivity ────────────────────
    info!("connecting to database…");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&cfg.database_url)
        .await?;

    info!("running migrations…");
    sqlx::migrate!("./migrations").run(&pool).await?;
    info!("migrations complete");

    // ── 2. Startup checks (run concurrently) ─────────────────────────────────
    let http = reqwest::Client::new();
    let (rpc_reachable, relayer_key_loaded) =
        tokio::join!(check_rpc_reachable(&cfg.rpc_urls, &http), async {
            check_relayer_key(&cfg.relayer_key_path)
        },);

    if !rpc_reachable {
        tracing::warn!("startup: one or more RPC endpoints unreachable");
    }
    if !relayer_key_loaded {
        tracing::warn!(
            path = cfg.relayer_key_path,
            "startup: relayer key not found or invalid"
        );
    }

    // ── 3. Construct repository adapters + modules ─────────────────────────────
    let relay_repo: Arc<dyn crate::domain::repository::RelayRepository> =
        Arc::new(db::PgRelayRepository::new(pool.clone()));
    let rebalance_repo = Arc::new(db::PgRebalanceRepository::new(pool.clone()));
    let watcher_repo = Arc::new(db::PgWatcherRepository::new(pool.clone()));
    let snapshot_repo = Arc::new(db::PgPoolSnapshotRepository::new(pool.clone()));

    let hot_path = HotPath::new(Arc::clone(&relay_repo), Arc::clone(&cfg), http.clone());
    let pool_manager = PoolManager::new(snapshot_repo, Arc::clone(&cfg), http.clone());
    let watcher = Watcher::new(watcher_repo, Arc::clone(&cfg), http.clone());
    let cold_path = ColdPath::new(rebalance_repo, Arc::clone(&cfg), http.clone());

    // ── 4. Spawn background tasks ─────────────────────────────────────────────
    Arc::clone(&hot_path).spawn_background();
    Arc::clone(&pool_manager).spawn_background();
    Arc::clone(&watcher).spawn_background();
    Arc::clone(&cold_path).spawn_background();

    // ── 5. Bind gRPC server ───────────────────────────────────────────────────
    let addr: SocketAddr = format!("0.0.0.0:{}", cfg.grpc_port).parse()?;
    info!(addr = %addr, "treasury gRPC server starting");

    let svc = TreasuryServer {
        pool,
        relay_repo,
        hot_path,
        pool_manager,
        watcher,
        relayer_key_loaded,
        rpc_reachable,
    };

    Server::builder()
        .add_service(TreasuryServiceServer::new(svc))
        .serve(addr)
        .await?;

    Ok(())
}
