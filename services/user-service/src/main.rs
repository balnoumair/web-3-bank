mod config;
mod db;
mod grpc;

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
    let pool = db::connect(&config.database_url).await?;

    tracing::info!("Running migrations...");
    sqlx::migrate!("src/db/migrations").run(&pool).await?;

    let addr: std::net::SocketAddr = config.grpc_addr.parse()?;

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("user.v1.UserService", ServingStatus::Serving)
        .await;

    tracing::info!(addr = %addr, "Starting gRPC server");
    Server::builder()
        .add_service(health_service)
        .add_service(UserServiceServer::new(UserServiceImpl { pool }))
        .serve(addr)
        .await?;

    Ok(())
}
