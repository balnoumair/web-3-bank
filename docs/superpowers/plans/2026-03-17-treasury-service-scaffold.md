# Treasury Service Scaffold Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up a compiling Rust binary at `services/treasury/` with a gRPC server, four Postgres migrations, config parsing, empty domain module stubs, and a live `HealthCheck` RPC.

**Architecture:** Proto-first (tonic). `tonic-build` + `protoc-bin-vendored` generate server traits from `proto/treasury.proto`; modules implement those traits. Startup runs migrations then three concurrent readiness checks (DB, RPC, relayer key) before binding the gRPC port. All domain RPCs return `UNIMPLEMENTED`; only `HealthCheck` is live.

**Tech Stack:** Rust 2021 edition, tokio 1, tonic 0.12, prost 0.13, sqlx 0.8 (postgres + rustls), alloy 0.3 (primitives), reqwest 0.12 (rustls-tls), envy 0.4, tracing 0.1

---

## File Structure

| File | Responsibility |
|------|---------------|
| `services/treasury/Cargo.toml` | Dependencies and build config |
| `services/treasury/build.rs` | tonic-build proto codegen (vendored protoc) |
| `services/treasury/proto/treasury.proto` | gRPC service contract — source of truth |
| `services/treasury/docker-compose.yml` | Local Postgres superuser container |
| `services/treasury/.env.example` | All env vars documented with format examples |
| `services/treasury/migrations/20260317000000_create_schema.sql` | `CREATE SCHEMA IF NOT EXISTS treasury` |
| `services/treasury/migrations/20260317000001_relay_logs.sql` | `treasury.relay_logs` table |
| `services/treasury/migrations/20260317000002_pool_snapshots.sql` | `treasury.pool_snapshots` table |
| `services/treasury/migrations/20260317000003_watcher_alerts.sql` | `treasury.watcher_alerts` table |
| `services/treasury/src/main.rs` | Binary entrypoint: startup validation → tonic server bind |
| `services/treasury/src/proto.rs` | `tonic::include_proto!("treasury")` — generated type module |
| `services/treasury/src/config.rs` | `Config::from_env()` — two-step construction + unit tests |
| `services/treasury/src/server.rs` | `TreasuryServer` struct + `TreasuryService` gRPC impl + check helpers + unit tests |
| `services/treasury/src/db/mod.rs` | `create_pool()`, `run_migrations()` + migration integration test |
| `services/treasury/src/hot_path/mod.rs` | Module declaration |
| `services/treasury/src/hot_path/listener.rs` | `HotPathListener` stub |
| `services/treasury/src/hot_path/relayer.rs` | `HotPathRelayer` stub |
| `services/treasury/src/cold_path/mod.rs` | Module declaration |
| `services/treasury/src/cold_path/rebalancer.rs` | `ColdPathRebalancer` stub |
| `services/treasury/src/watcher/mod.rs` | Module declaration |
| `services/treasury/src/watcher/verifier.rs` | `WatcherVerifier` stub |
| `services/treasury/src/pool/mod.rs` | Module declaration |
| `services/treasury/src/pool/manager.rs` | `PoolManager` stub |

---

## Chunk 1: Project Skeleton

### Task 1: Initialize Cargo workspace, build script, and proto

**Files:**
- Create: `services/treasury/Cargo.toml`
- Create: `services/treasury/build.rs`
- Create: `services/treasury/proto/treasury.proto`
- Create: `services/treasury/src/main.rs` (minimal — replaced in Task 6)

- [ ] **Step 1: Create the directory tree**

```bash
mkdir -p services/treasury/proto
mkdir -p services/treasury/src
mkdir -p services/treasury/migrations
```

- [ ] **Step 2: Write `Cargo.toml`**

```toml
# services/treasury/Cargo.toml
[package]
name = "treasury"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "treasury"
path = "src/main.rs"

[dependencies]
tokio       = { version = "1",    features = ["full"] }
tonic       = "0.12"
prost       = "0.13"
sqlx        = { version = "0.8",  features = ["postgres", "runtime-tokio-rustls", "macros", "migrate"] }
alloy       = { version = "0.3",  features = ["primitives"] }
tracing     = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
envy        = "0.4"
serde       = { version = "1",    features = ["derive"] }
serde_json  = "1"
reqwest     = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
thiserror   = "1"

[build-dependencies]
tonic-build          = "0.12"
protoc-bin-vendored  = "3"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Write `build.rs`**

```rust
// services/treasury/build.rs
fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored: could not find vendored protoc binary");
    std::env::set_var("PROTOC", &protoc);
    tonic_build::compile_protos("proto/treasury.proto")
        .expect("failed to compile proto/treasury.proto");
}
```

- [ ] **Step 4: Write `proto/treasury.proto`**

```protobuf
// services/treasury/proto/treasury.proto
syntax = "proto3";
package treasury;

service TreasuryService {
  rpc HealthCheck(HealthCheckRequest) returns (HealthCheckResponse);

  // Hot path — implemented in Task 05
  rpc GetRelayStatus(GetRelayStatusRequest) returns (GetRelayStatusResponse);

  // Pool management — implemented in Task 05/10
  rpc GetPoolDepth(GetPoolDepthRequest) returns (GetPoolDepthResponse);

  // Watcher — implemented in Task 06
  rpc GetWatcherAlerts(GetWatcherAlertsRequest) returns (GetWatcherAlertsResponse);
}

message HealthCheckRequest {}
message HealthCheckResponse {
  enum Status {
    UNKNOWN     = 0;  // proto3 zero value — never returned at runtime
    SERVING     = 1;
    NOT_SERVING = 2;
  }
  Status status          = 1;
  bool   db_connected    = 2;
  bool   rpc_reachable   = 3;
  bool   relayer_key_loaded = 4;
}

message GetRelayStatusRequest  { string source_event_hash = 1; }
message GetRelayStatusResponse { string status = 1; }

message GetPoolDepthRequest    { uint64 chain_id = 1; }
message GetPoolDepthResponse   { string depth_wei = 1; }

message GetWatcherAlertsRequest  { uint32 limit = 1; }
message GetWatcherAlertsResponse { repeated string alert_ids = 1; }
```

- [ ] **Step 5: Write minimal `src/main.rs`** (placeholder — replaced in Task 6)

```rust
// services/treasury/src/main.rs
fn main() {}
```

- [ ] **Step 6: Verify the project compiles**

```bash
cd services/treasury
cargo build
```

Expected: compilation succeeds. First run downloads dependencies (~2 min). If you see a protoc error, verify `protoc-bin-vendored` is listed in `[build-dependencies]`.

- [ ] **Step 7: Commit**

```bash
git add services/treasury/
git commit -m "feat(treasury): initialize cargo workspace with tonic proto scaffold"
```

---

### Task 2: Docker Compose and environment template

**Files:**
- Create: `services/treasury/docker-compose.yml`
- Create: `services/treasury/.env.example`

- [ ] **Step 1: Write `docker-compose.yml`**

```yaml
# services/treasury/docker-compose.yml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: web3bank
      POSTGRES_USER: postgres       # superuser — required for sqlx::test CREATEDB
      POSTGRES_PASSWORD: postgres
    ports:
      - "5432:5432"
    volumes:
      - treasury_pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 3s
      retries: 5

volumes:
  treasury_pgdata:
```

- [ ] **Step 2: Write `.env.example`**

```bash
# services/treasury/.env.example
#
# Copy to .env and fill in values before running `cargo run`.
# Never commit .env to git.

# Postgres connection string
# Format: postgres://<user>:<password>@<host>:<port>/<database>
DATABASE_URL=postgres://postgres:postgres@localhost:5432/web3bank

# gRPC port (default: 50051)
GRPC_PORT=50051

# Path to the relayer private key file.
# File must contain a single 0x-prefixed 64-hex-character string, e.g.:
#   0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890
RELAYER_KEY_PATH=/path/to/relayer.key

# RouteReceiver.sol address on Base Sepolia (0x-prefixed checksummed address)
ROUTE_RECEIVER_ADDRESS=0x0000000000000000000000000000000000000000

# JSON object mapping chain_id (integer) to RPC URL string
# Example: {"1":"https://eth-mainnet.example.com","8453":"https://base-mainnet.example.com"}
RPC_URLS_JSON={"1":"https://your-eth-rpc.example.com"}

# JSON object mapping chain_id (integer) to BankContract address string
# Example: {"1":"0xABC...","8453":"0xDEF..."}
CONTRACT_ADDRESSES_JSON={"1":"0x0000000000000000000000000000000000000000"}
```

- [ ] **Step 3: Verify Postgres starts**

```bash
cd services/treasury
docker compose up -d postgres
docker compose ps
```

Expected: `postgres` container shows `healthy` status within 30 seconds.

- [ ] **Step 4: Commit**

```bash
git add services/treasury/docker-compose.yml services/treasury/.env.example
git commit -m "feat(treasury): add docker-compose postgres and env template"
```

---

## Chunk 2: Database

### Task 3: Migrations and DB pool setup

**Files:**
- Create: `services/treasury/migrations/20260317000000_create_schema.sql`
- Create: `services/treasury/migrations/20260317000001_relay_logs.sql`
- Create: `services/treasury/migrations/20260317000002_pool_snapshots.sql`
- Create: `services/treasury/migrations/20260317000003_watcher_alerts.sql`
- Create: `services/treasury/src/db/mod.rs`

- [ ] **Step 1: Write the failing integration test first**

Create `services/treasury/src/db/mod.rs` with only the test:

```rust
// services/treasury/src/db/mod.rs
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    todo!("implement in Step 4")
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    todo!("implement in Step 4")
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    // Requires: DATABASE_URL=postgres://postgres:postgres@localhost:5432/web3bank
    // Start postgres first: docker compose up -d postgres
    #[sqlx::test(migrations = "migrations")]
    async fn test_all_tables_exist(pool: PgPool) -> sqlx::Result<()> {
        // relay_logs table exists and is empty
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM treasury.relay_logs")
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0, "relay_logs should be empty");

        // pool_snapshots table exists and is empty
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM treasury.pool_snapshots")
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0, "pool_snapshots should be empty");

        // watcher_alerts table exists and is empty
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM treasury.watcher_alerts")
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0, "watcher_alerts should be empty");

        Ok(())
    }
}
```

Also add `mod db;` to `src/main.rs`:

```rust
// services/treasury/src/main.rs
mod db;
fn main() {}
```

- [ ] **Step 2: Run the test — expect compile error on `todo!()`**

```bash
cd services/treasury
DATABASE_URL=postgres://postgres:postgres@localhost:5432/web3bank \
  cargo test -- --test-threads=1 2>&1 | head -30
```

Expected: test fails to compile or panics on `todo!()`. That's correct — we haven't written the implementation yet.

- [ ] **Step 3: Write migration 0 — schema creation**

```sql
-- services/treasury/migrations/20260317000000_create_schema.sql
CREATE SCHEMA IF NOT EXISTS treasury;
```

- [ ] **Step 4: Write migration 1 — relay_logs**

```sql
-- services/treasury/migrations/20260317000001_relay_logs.sql
CREATE TABLE treasury.relay_logs (
    id                BIGSERIAL    PRIMARY KEY,
    source_event_hash TEXT         NOT NULL UNIQUE,
    dest_tx_hash      TEXT,
    source_chain_id   BIGINT       NOT NULL,
    dest_chain_id     BIGINT       NOT NULL,
    recipient         TEXT         NOT NULL,
    amount_wei        NUMERIC(78)  NOT NULL,
    status            TEXT         NOT NULL DEFAULT 'pending',
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ  NOT NULL DEFAULT now()
);
```

- [ ] **Step 5: Write migration 2 — pool_snapshots**

```sql
-- services/treasury/migrations/20260317000002_pool_snapshots.sql
CREATE TABLE treasury.pool_snapshots (
    id          BIGSERIAL   PRIMARY KEY,
    chain_id    BIGINT      NOT NULL,
    depth_wei   NUMERIC(78) NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

- [ ] **Step 6: Write migration 3 — watcher_alerts**

```sql
-- services/treasury/migrations/20260317000003_watcher_alerts.sql
CREATE TABLE treasury.watcher_alerts (
    id                BIGSERIAL   PRIMARY KEY,
    source_event_hash TEXT        NOT NULL,
    alert_type        TEXT        NOT NULL,
    detail            TEXT,
    resolved          BOOLEAN     NOT NULL DEFAULT false,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

- [ ] **Step 7: Implement `create_pool` and `run_migrations` in `db/mod.rs`**

```rust
// services/treasury/src/db/mod.rs
use sqlx::PgPool;

/// Create a connection pool for the given DATABASE_URL.
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPool::connect(database_url).await
}

/// Run all pending migrations from the `migrations/` directory.
/// Path is relative to Cargo.toml.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    // Requires: DATABASE_URL=postgres://postgres:postgres@localhost:5432/web3bank
    // Start postgres first: docker compose up -d postgres
    #[sqlx::test(migrations = "migrations")]
    async fn test_all_tables_exist(pool: PgPool) -> sqlx::Result<()> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM treasury.relay_logs")
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0, "relay_logs should be empty");

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM treasury.pool_snapshots")
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0, "pool_snapshots should be empty");

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM treasury.watcher_alerts")
                .fetch_one(&pool)
                .await?;
        assert_eq!(count, 0, "watcher_alerts should be empty");

        Ok(())
    }
}
```

- [ ] **Step 8: Ensure Postgres is running, then run the test**

```bash
cd services/treasury
docker compose up -d postgres

# Wait for healthy status
docker compose ps

DATABASE_URL=postgres://postgres:postgres@localhost:5432/web3bank \
  cargo test db::tests -- --test-threads=1 --nocapture
```

Expected output:
```
running 1 test
test db::tests::test_all_tables_exist ... ok

test result: ok. 1 passed; 0 failed
```

- [ ] **Step 9: Commit**

```bash
git add services/treasury/migrations/ services/treasury/src/db/mod.rs services/treasury/src/main.rs
git commit -m "feat(treasury): add postgres migrations and db pool setup"
```

---

## Chunk 3: Configuration

### Task 4: Config parsing

**Files:**
- Create: `services/treasury/src/config.rs`

- [ ] **Step 1: Write the failing unit tests first**

Create `services/treasury/src/config.rs` with only the test module:

```rust
// services/treasury/src/config.rs
pub struct Config {}  // placeholder

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        todo!("implement Config::from_env()")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: env var tests must be run with --test-threads=1 to avoid races.
    // Run: cargo test config::tests -- --test-threads=1

    #[test]
    fn test_config_parses_all_fields() {
        std::env::set_var("DATABASE_URL", "postgres://postgres:postgres@localhost:5432/web3bank");
        std::env::set_var("GRPC_PORT", "9090");
        std::env::set_var("RELAYER_KEY_PATH", "/tmp/test-relayer.key");
        std::env::set_var(
            "ROUTE_RECEIVER_ADDRESS",
            "0x0000000000000000000000000000000000000001",
        );
        std::env::set_var(
            "RPC_URLS_JSON",
            r#"{"1":"https://eth.example.com","8453":"https://base.example.com"}"#,
        );
        std::env::set_var(
            "CONTRACT_ADDRESSES_JSON",
            r#"{"1":"0x0000000000000000000000000000000000000002","8453":"0x0000000000000000000000000000000000000003"}"#,
        );

        let config = Config::from_env().expect("Config::from_env should succeed");

        assert_eq!(config.grpc_port, 9090);
        assert_eq!(
            config.database_url,
            "postgres://postgres:postgres@localhost:5432/web3bank"
        );
        assert_eq!(config.relayer_key_path, "/tmp/test-relayer.key");
        assert_eq!(config.rpc_urls.len(), 2);
        assert!(config.rpc_urls.contains_key(&1u64));
        assert!(config.rpc_urls.contains_key(&8453u64));
        assert_eq!(config.contract_addresses.len(), 2);

        // Cleanup
        for var in &[
            "DATABASE_URL", "GRPC_PORT", "RELAYER_KEY_PATH",
            "ROUTE_RECEIVER_ADDRESS", "RPC_URLS_JSON", "CONTRACT_ADDRESSES_JSON",
        ] {
            std::env::remove_var(var);
        }
    }

    #[test]
    fn test_grpc_port_defaults_to_50051() {
        std::env::set_var("DATABASE_URL", "postgres://...");
        std::env::set_var("RELAYER_KEY_PATH", "/tmp/k");
        std::env::set_var("ROUTE_RECEIVER_ADDRESS", "0x0000000000000000000000000000000000000001");
        std::env::set_var("RPC_URLS_JSON", r#"{"1":"https://eth.example.com"}"#);
        std::env::set_var("CONTRACT_ADDRESSES_JSON", r#"{"1":"0x0000000000000000000000000000000000000002"}"#);
        std::env::remove_var("GRPC_PORT");

        let config = Config::from_env().expect("should succeed without GRPC_PORT");
        assert_eq!(config.grpc_port, 50051, "default port should be 50051");

        for var in &[
            "DATABASE_URL", "RELAYER_KEY_PATH", "ROUTE_RECEIVER_ADDRESS",
            "RPC_URLS_JSON", "CONTRACT_ADDRESSES_JSON",
        ] {
            std::env::remove_var(var);
        }
    }

    #[test]
    fn test_missing_required_var_returns_error() {
        // Remove a required var and expect an error
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("RELAYER_KEY_PATH");
        std::env::remove_var("ROUTE_RECEIVER_ADDRESS");
        std::env::remove_var("RPC_URLS_JSON");
        std::env::remove_var("CONTRACT_ADDRESSES_JSON");

        let result = Config::from_env();
        assert!(result.is_err(), "should fail when required vars are missing");
    }
}
```

Add `mod config;` to `src/main.rs`:

```rust
// services/treasury/src/main.rs
mod config;
mod db;
fn main() {}
```

- [ ] **Step 2: Run — expect panic on `todo!()`**

```bash
cd services/treasury
cargo test config::tests -- --test-threads=1 2>&1 | head -20
```

Expected: tests panic on `todo!()`. Correct — we haven't implemented yet.

- [ ] **Step 3: Implement `config.rs`**

```rust
// services/treasury/src/config.rs
use alloy::primitives::Address;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("env deserialization failed: {0}")]
    Envy(#[from] envy::Error),
    #[error("required env var {0} is missing")]
    Missing(String),
    #[error("invalid value for env var {0}: {1}")]
    Invalid(String, String),
}

/// Internal helper — only plain string/int scalar fields that envy can deserialize.
#[derive(serde::Deserialize)]
struct ScalarConfig {
    database_url: String,
    #[serde(default = "default_grpc_port")]
    grpc_port: u16,
    relayer_key_path: String,
}

fn default_grpc_port() -> u16 {
    50051
}

/// Service configuration. Construct via `Config::from_env()`.
#[derive(Debug)]
pub struct Config {
    pub database_url: String,
    pub grpc_port: u16,
    /// chain_id → RPC URL (from RPC_URLS_JSON)
    pub rpc_urls: HashMap<u64, String>,
    /// chain_id → BankContract address (from CONTRACT_ADDRESSES_JSON)
    pub contract_addresses: HashMap<u64, Address>,
    /// RouteReceiver.sol address (from ROUTE_RECEIVER_ADDRESS)
    pub route_receiver_address: Address,
    /// Path to relayer private key file (from RELAYER_KEY_PATH)
    pub relayer_key_path: String,
}

impl Config {
    /// Load configuration from environment variables.
    /// Fails fast if any required variable is missing or invalid.
    pub fn from_env() -> Result<Self, ConfigError> {
        // Step 1: scalar fields via envy
        let scalar: ScalarConfig = envy::from_env()?;

        // Step 2: ROUTE_RECEIVER_ADDRESS
        let route_receiver_address = {
            let raw = std::env::var("ROUTE_RECEIVER_ADDRESS")
                .map_err(|_| ConfigError::Missing("ROUTE_RECEIVER_ADDRESS".into()))?;
            Address::from_str(&raw)
                .map_err(|e| ConfigError::Invalid("ROUTE_RECEIVER_ADDRESS".into(), e.to_string()))?
        };

        // Step 3: RPC_URLS_JSON — {"chain_id": "url", ...}
        let rpc_urls: HashMap<u64, String> = {
            let raw = std::env::var("RPC_URLS_JSON")
                .map_err(|_| ConfigError::Missing("RPC_URLS_JSON".into()))?;
            serde_json::from_str(&raw)
                .map_err(|e| ConfigError::Invalid("RPC_URLS_JSON".into(), e.to_string()))?
        };

        // Step 4: CONTRACT_ADDRESSES_JSON — {"chain_id": "0x...", ...}
        let contract_addresses: HashMap<u64, Address> = {
            let raw = std::env::var("CONTRACT_ADDRESSES_JSON")
                .map_err(|_| ConfigError::Missing("CONTRACT_ADDRESSES_JSON".into()))?;
            let str_map: HashMap<u64, String> = serde_json::from_str(&raw)
                .map_err(|e| ConfigError::Invalid("CONTRACT_ADDRESSES_JSON".into(), e.to_string()))?;
            str_map
                .into_iter()
                .map(|(chain_id, addr_str)| {
                    Address::from_str(&addr_str)
                        .map(|addr| (chain_id, addr))
                        .map_err(|e| {
                            ConfigError::Invalid(
                                "CONTRACT_ADDRESSES_JSON".into(),
                                format!("chain {chain_id}: {e}"),
                            )
                        })
                })
                .collect::<Result<HashMap<_, _>, _>>()?
        };

        Ok(Config {
            database_url: scalar.database_url,
            grpc_port: scalar.grpc_port,
            rpc_urls,
            contract_addresses,
            route_receiver_address,
            relayer_key_path: scalar.relayer_key_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: env var tests must be run with --test-threads=1 to avoid races.
    // Run: cargo test config::tests -- --test-threads=1

    #[test]
    fn test_config_parses_all_fields() {
        std::env::set_var("DATABASE_URL", "postgres://postgres:postgres@localhost:5432/web3bank");
        std::env::set_var("GRPC_PORT", "9090");
        std::env::set_var("RELAYER_KEY_PATH", "/tmp/test-relayer.key");
        std::env::set_var(
            "ROUTE_RECEIVER_ADDRESS",
            "0x0000000000000000000000000000000000000001",
        );
        std::env::set_var(
            "RPC_URLS_JSON",
            r#"{"1":"https://eth.example.com","8453":"https://base.example.com"}"#,
        );
        std::env::set_var(
            "CONTRACT_ADDRESSES_JSON",
            r#"{"1":"0x0000000000000000000000000000000000000002","8453":"0x0000000000000000000000000000000000000003"}"#,
        );

        let config = Config::from_env().expect("Config::from_env should succeed");

        assert_eq!(config.grpc_port, 9090);
        assert_eq!(
            config.database_url,
            "postgres://postgres:postgres@localhost:5432/web3bank"
        );
        assert_eq!(config.relayer_key_path, "/tmp/test-relayer.key");
        assert_eq!(config.rpc_urls.len(), 2);
        assert!(config.rpc_urls.contains_key(&1u64));
        assert!(config.rpc_urls.contains_key(&8453u64));
        assert_eq!(config.contract_addresses.len(), 2);

        for var in &[
            "DATABASE_URL", "GRPC_PORT", "RELAYER_KEY_PATH",
            "ROUTE_RECEIVER_ADDRESS", "RPC_URLS_JSON", "CONTRACT_ADDRESSES_JSON",
        ] {
            std::env::remove_var(var);
        }
    }

    #[test]
    fn test_grpc_port_defaults_to_50051() {
        std::env::set_var("DATABASE_URL", "postgres://...");
        std::env::set_var("RELAYER_KEY_PATH", "/tmp/k");
        std::env::set_var("ROUTE_RECEIVER_ADDRESS", "0x0000000000000000000000000000000000000001");
        std::env::set_var("RPC_URLS_JSON", r#"{"1":"https://eth.example.com"}"#);
        std::env::set_var("CONTRACT_ADDRESSES_JSON", r#"{"1":"0x0000000000000000000000000000000000000002"}"#);
        std::env::remove_var("GRPC_PORT");

        let config = Config::from_env().expect("should succeed without GRPC_PORT");
        assert_eq!(config.grpc_port, 50051);

        for var in &[
            "DATABASE_URL", "RELAYER_KEY_PATH", "ROUTE_RECEIVER_ADDRESS",
            "RPC_URLS_JSON", "CONTRACT_ADDRESSES_JSON",
        ] {
            std::env::remove_var(var);
        }
    }

    #[test]
    fn test_missing_required_var_returns_error() {
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("RELAYER_KEY_PATH");
        std::env::remove_var("ROUTE_RECEIVER_ADDRESS");
        std::env::remove_var("RPC_URLS_JSON");
        std::env::remove_var("CONTRACT_ADDRESSES_JSON");

        let result = Config::from_env();
        assert!(result.is_err(), "should fail when required vars are missing");
    }
}
```

- [ ] **Step 4: Run the config tests**

```bash
cd services/treasury
cargo test config::tests -- --test-threads=1 --nocapture
```

Expected output:
```
running 3 tests
test config::tests::test_missing_required_var_returns_error ... ok
test config::tests::test_grpc_port_defaults_to_50051 ... ok
test config::tests::test_config_parses_all_fields ... ok

test result: ok. 3 passed; 0 failed
```

- [ ] **Step 5: Commit**

```bash
git add services/treasury/src/config.rs services/treasury/src/main.rs
git commit -m "feat(treasury): implement Config::from_env() with two-step construction"
```

---

## Chunk 4: Module Stubs and gRPC Server

### Task 5: Domain module stubs

**Files:**
- Create: `services/treasury/src/hot_path/mod.rs`
- Create: `services/treasury/src/hot_path/listener.rs`
- Create: `services/treasury/src/hot_path/relayer.rs`
- Create: `services/treasury/src/cold_path/mod.rs`
- Create: `services/treasury/src/cold_path/rebalancer.rs`
- Create: `services/treasury/src/watcher/mod.rs`
- Create: `services/treasury/src/watcher/verifier.rs`
- Create: `services/treasury/src/pool/mod.rs`
- Create: `services/treasury/src/pool/manager.rs`

- [ ] **Step 1: Write all stub files**

```rust
// services/treasury/src/hot_path/mod.rs
pub mod listener;
pub mod relayer;
```

```rust
// services/treasury/src/hot_path/listener.rs
/// Listens for HotPathInitiated events on all active chains.
/// Implementation: Task 05
pub struct HotPathListener;
```

```rust
// services/treasury/src/hot_path/relayer.rs
/// Submits releaseHotPath transactions on destination chains.
/// Implementation: Task 05
pub struct HotPathRelayer;
```

```rust
// services/treasury/src/cold_path/mod.rs
pub mod rebalancer;
```

```rust
// services/treasury/src/cold_path/rebalancer.rs
/// Executes CCIP burn-and-mint rebalancing operations.
/// Implementation: Task 10
pub struct ColdPathRebalancer;
```

```rust
// services/treasury/src/watcher/mod.rs
pub mod verifier;
```

```rust
// services/treasury/src/watcher/verifier.rs
/// Cross-references HotPathReleased events against HotPathInitiated events.
/// Triggers contract pause on mismatch.
/// Implementation: Task 06
pub struct WatcherVerifier;
```

```rust
// services/treasury/src/pool/mod.rs
pub mod manager;
```

```rust
// services/treasury/src/pool/manager.rs
/// Tracks real-time pool depths across all chains.
/// Enforces minimum liquidity thresholds.
/// Implementation: Task 05
pub struct PoolManager;
```

- [ ] **Step 2: Add module declarations to `src/main.rs`**

```rust
// services/treasury/src/main.rs
mod cold_path;
mod config;
mod db;
mod hot_path;
mod pool;
mod watcher;
fn main() {}
```

- [ ] **Step 3: Verify all stubs compile**

```bash
cd services/treasury
cargo build
```

Expected: compiles without errors or warnings about unused items.

- [ ] **Step 4: Commit**

```bash
git add services/treasury/src/hot_path/ services/treasury/src/cold_path/ \
        services/treasury/src/watcher/ services/treasury/src/pool/ \
        services/treasury/src/main.rs
git commit -m "feat(treasury): add domain module stubs (hot_path, cold_path, watcher, pool)"
```

---

### Task 6: gRPC server with HealthCheck

**Files:**
- Create: `services/treasury/src/proto.rs`
- Create: `services/treasury/src/server.rs`
- Modify: `services/treasury/src/main.rs` (replace placeholder)

- [ ] **Step 1: Write failing unit tests for relayer key validation**

Create `services/treasury/src/server.rs` with only the key check function and tests:

```rust
// services/treasury/src/server.rs
pub async fn check_relayer_key(path: &str) -> bool {
    todo!("implement key validation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_valid_key_with_0x_prefix() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        )
        .unwrap();
        assert!(check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_valid_key_without_0x_prefix() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        )
        .unwrap();
        assert!(check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_invalid_key_non_hex() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "not-a-valid-private-key").unwrap();
        assert!(!check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_invalid_key_too_short() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "0xabcdef").unwrap();
        assert!(!check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_missing_key_file() {
        assert!(!check_relayer_key("/nonexistent/path/relayer.key").await);
    }
}
```

Add `mod server;` to `src/main.rs`:

```rust
// services/treasury/src/main.rs
mod cold_path;
mod config;
mod db;
mod hot_path;
mod pool;
mod server;
mod watcher;
fn main() {}
```

- [ ] **Step 2: Run — expect compile error or panic on `todo!()`**

```bash
cd services/treasury
cargo test server::tests -- --nocapture 2>&1 | head -20
```

Expected: all 5 tests panic on `todo!()`. Correct.

- [ ] **Step 3: Implement `check_relayer_key` and run tests**

Replace the `check_relayer_key` function:

```rust
// In services/treasury/src/server.rs — replace the todo!() stub
pub async fn check_relayer_key(path: &str) -> bool {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => {
            let trimmed = content.trim();
            let hex = trimmed.strip_prefix("0x").unwrap_or(trimmed);
            hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
        }
        Err(e) => {
            tracing::warn!("Failed to read relayer key at {}: {}", path, e);
            false
        }
    }
}
```

```bash
cd services/treasury
cargo test server::tests -- --nocapture
```

Expected:
```
running 5 tests
test server::tests::test_valid_key_with_0x_prefix ... ok
test server::tests::test_valid_key_without_0x_prefix ... ok
test server::tests::test_invalid_key_non_hex ... ok
test server::tests::test_invalid_key_too_short ... ok
test server::tests::test_missing_key_file ... ok

test result: ok. 5 passed; 0 failed
```

- [ ] **Step 4: Write `src/proto.rs`**

```rust
// services/treasury/src/proto.rs
tonic::include_proto!("treasury");
```

- [ ] **Step 5: Complete `src/server.rs` with full gRPC service impl**

Replace the entire `server.rs` with the complete implementation:

```rust
// services/treasury/src/server.rs
use std::collections::HashMap;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::config::Config;
use crate::proto::{
    health_check_response, GetPoolDepthRequest, GetPoolDepthResponse,
    GetRelayStatusRequest, GetRelayStatusResponse, GetWatcherAlertsRequest,
    GetWatcherAlertsResponse, HealthCheckRequest, HealthCheckResponse,
    treasury_service_server::TreasuryService,
};

/// The gRPC service implementation. Holds runtime dependencies.
pub struct TreasuryServer {
    pool: sqlx::PgPool,
    config: Arc<Config>,
    http_client: reqwest::Client,
}

impl TreasuryServer {
    pub fn new(pool: sqlx::PgPool, config: Arc<Config>, http_client: reqwest::Client) -> Self {
        Self { pool, config, http_client }
    }
}

/// Check DB connectivity with a lightweight ping.
pub async fn check_db(pool: &sqlx::PgPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

/// Check that every configured chain RPC responds to eth_blockNumber.
/// Returns true only if ALL chains respond successfully.
pub async fn check_rpcs(rpc_urls: &HashMap<u64, String>, client: &reqwest::Client) -> bool {
    for (chain_id, url) in rpc_urls {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method":  "eth_blockNumber",
            "params":  [],
            "id":      1
        });
        match client.post(url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => {
                warn!("RPC chain {} returned HTTP {}: {}", chain_id, resp.status(), url);
                return false;
            }
            Err(e) => {
                warn!("RPC chain {} unreachable at {}: {}", chain_id, url, e);
                return false;
            }
        }
    }
    true
}

/// Check the relayer key file: must be a 0x-prefixed (or bare) 64-char hex string.
pub async fn check_relayer_key(path: &str) -> bool {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => {
            let trimmed = content.trim();
            let hex = trimmed.strip_prefix("0x").unwrap_or(trimmed);
            hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
        }
        Err(e) => {
            warn!("Failed to read relayer key at {}: {}", path, e);
            false
        }
    }
}

#[tonic::async_trait]
impl TreasuryService for TreasuryServer {
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let (db_ok, rpc_ok, key_ok) = tokio::join!(
            check_db(&self.pool),
            check_rpcs(&self.config.rpc_urls, &self.http_client),
            check_relayer_key(&self.config.relayer_key_path),
        );

        let overall = if db_ok && rpc_ok && key_ok {
            health_check_response::Status::Serving
        } else {
            health_check_response::Status::NotServing
        };

        Ok(Response::new(HealthCheckResponse {
            status: overall as i32,
            db_connected: db_ok,
            rpc_reachable: rpc_ok,
            relayer_key_loaded: key_ok,
        }))
    }

    async fn get_relay_status(
        &self,
        _: Request<GetRelayStatusRequest>,
    ) -> Result<Response<GetRelayStatusResponse>, Status> {
        Err(Status::unimplemented("GetRelayStatus: implemented in Task 05"))
    }

    async fn get_pool_depth(
        &self,
        _: Request<GetPoolDepthRequest>,
    ) -> Result<Response<GetPoolDepthResponse>, Status> {
        Err(Status::unimplemented("GetPoolDepth: implemented in Task 05/10"))
    }

    async fn get_watcher_alerts(
        &self,
        _: Request<GetWatcherAlertsRequest>,
    ) -> Result<Response<GetWatcherAlertsResponse>, Status> {
        Err(Status::unimplemented("GetWatcherAlerts: implemented in Task 06"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_valid_key_with_0x_prefix() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        )
        .unwrap();
        assert!(check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_valid_key_without_0x_prefix() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        )
        .unwrap();
        assert!(check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_invalid_key_non_hex() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "not-a-valid-private-key").unwrap();
        assert!(!check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_invalid_key_too_short() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "0xabcdef").unwrap();
        assert!(!check_relayer_key(f.path().to_str().unwrap()).await);
    }

    #[tokio::test]
    async fn test_missing_key_file() {
        assert!(!check_relayer_key("/nonexistent/path/relayer.key").await);
    }
}
```

- [ ] **Step 6: Write the final `src/main.rs`**

```rust
// services/treasury/src/main.rs
mod cold_path;
mod config;
mod db;
mod hot_path;
mod pool;
mod proto;
mod server;
mod watcher;

use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load and validate configuration
    let config = config::Config::from_env()?;
    tracing::info!("Configuration loaded (gRPC port: {})", config.grpc_port);

    // Phase A: Run migrations (sequential — must complete before readiness checks)
    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;
    tracing::info!("Migrations complete");

    // Phase B: Concurrent readiness checks
    let http_client = reqwest::Client::new();
    let (db_ok, rpc_ok, key_ok) = tokio::join!(
        server::check_db(&pool),
        server::check_rpcs(&config.rpc_urls, &http_client),
        server::check_relayer_key(&config.relayer_key_path),
    );

    if !db_ok || !rpc_ok || !key_ok {
        if !db_ok {
            tracing::error!("startup FAILED: DB connectivity check");
        }
        if !rpc_ok {
            tracing::error!("startup FAILED: RPC reachability check");
        }
        if !key_ok {
            tracing::error!(
                "startup FAILED: relayer key check (path: {})",
                config.relayer_key_path
            );
        }
        std::process::exit(1);
    }
    tracing::info!("All startup checks passed");

    // Bind gRPC server
    let addr = format!("0.0.0.0:{}", config.grpc_port).parse()?;
    let treasury_server = server::TreasuryServer::new(pool, Arc::new(config), http_client);

    tracing::info!("Treasury gRPC server listening on {}", addr);

    tonic::transport::Server::builder()
        .add_service(
            proto::treasury_service_server::TreasuryServiceServer::new(treasury_server),
        )
        .serve(addr)
        .await?;

    Ok(())
}
```

- [ ] **Step 7: Run the server-level unit tests**

```bash
cd services/treasury
cargo test server::tests -- --nocapture
```

Expected:
```
running 5 tests
test server::tests::test_valid_key_with_0x_prefix ... ok
test server::tests::test_valid_key_without_0x_prefix ... ok
test server::tests::test_invalid_key_non_hex ... ok
test server::tests::test_invalid_key_too_short ... ok
test server::tests::test_missing_key_file ... ok

test result: ok. 5 passed; 0 failed
```

- [ ] **Step 8: Run the full test suite**

```bash
cd services/treasury
DATABASE_URL=postgres://postgres:postgres@localhost:5432/web3bank \
  cargo test -- --test-threads=1 --nocapture
```

Expected: all tests pass (config::tests × 3, db::tests × 1, server::tests × 5).

- [ ] **Step 9: Build the release binary**

```bash
cd services/treasury
cargo build
```

Expected: compiles without errors.

- [ ] **Step 10: Manual HealthCheck verification**

Install grpcurl if not present: `brew install grpcurl`

In one terminal, set up a minimal `.env` and start the server:

```bash
cd services/treasury
# Create a dummy relayer key file
echo "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890" > /tmp/test-relayer.key

# Start with docker postgres already running
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank"
export RELAYER_KEY_PATH="/tmp/test-relayer.key"
export ROUTE_RECEIVER_ADDRESS="0x0000000000000000000000000000000000000001"
export RPC_URLS_JSON='{"1":"https://eth-mainnet.g.alchemy.com/v2/demo"}'
export CONTRACT_ADDRESSES_JSON='{"1":"0x0000000000000000000000000000000000000002"}'

cargo run
```

In a second terminal:

```bash
cd services/treasury
grpcurl -plaintext -proto proto/treasury.proto \
  -d '{}' localhost:50051 treasury.TreasuryService/HealthCheck
```

Expected: JSON response with `status` field (either `"SERVING"` or `"NOT_SERVING"` depending on whether the RPC URL and key resolve).

```bash
# Verify stub RPCs return UNIMPLEMENTED
grpcurl -plaintext -proto proto/treasury.proto \
  -d '{"source_event_hash":"0xabc"}' localhost:50051 treasury.TreasuryService/GetRelayStatus
```

Expected: `ERROR: Code: Unimplemented, Message: GetRelayStatus: implemented in Task 05`

- [ ] **Step 11: Commit**

```bash
git add services/treasury/src/proto.rs services/treasury/src/server.rs services/treasury/src/main.rs
git commit -m "feat(treasury): implement gRPC server with live HealthCheck RPC"
```
