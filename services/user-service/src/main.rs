mod config;
mod db;
pub mod domain;
mod grpc;

use std::sync::Arc;

use grpc::user_service::UserServiceImpl;
use grpc::UserServiceServer;
use tonic::transport::Server;
use tonic_health::ServingStatus;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::from_env().map_err(|e| {
        eprintln!("Configuration error: {e}");
        e
    })?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Connecting to database...");
    let pool = db::connect(&config.database_url).await.map_err(|e| {
        tracing::error!("Failed to connect to database: {e}");
        e
    })?;

    tracing::info!("Running migrations...");
    sqlx::migrate!("src/db/migrations")
        .run(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to run migrations: {e}");
            e
        })?;

    let addr: std::net::SocketAddr = config.grpc_addr.parse()?;

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("user.UserService", ServingStatus::Serving)
        .await;

    let user_repo = Arc::new(db::PgUserRepository::new(pool.clone()));
    let credential_repo = Arc::new(db::PgCredentialRepository::new(pool));

    tracing::info!(addr = %addr, "Starting gRPC server");
    Server::builder()
        .add_service(health_service)
        .add_service(UserServiceServer::new(UserServiceImpl {
            user_repo,
            credential_repo,
        }))
        .serve(addr)
        .await?;

    Ok(())
}
