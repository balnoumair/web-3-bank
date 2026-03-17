# Treasury Service Scaffold — Design Spec

**Date:** 2026-03-17
**Task:** 04 — Treasury Service Scaffold
**Status:** Approved

---

## Overview

Set up the Rust project structure for the Treasury Service at `services/treasury/`. This is a Phase 1 scaffold task — it produces a compiling binary with a working gRPC server, database migrations, and startup validation. All domain modules are stubbed; business logic is filled in by Tasks 05, 06, and 10.

**Approach:** Proto-first. The `.proto` file is the primary contract. `tonic-build` generates server traits from it; modules implement those traits. This locks in the BFF-facing API surface before Phase 2 tasks begin.

---

## 1. Repository Layout

The service lives at `services/treasury/` inside the monorepo root as a standalone Cargo workspace. It is not part of the pnpm workspace. The monorepo root does not contain a `Cargo.toml` (it is a pnpm/Node monorepo), so no workspace exclusion is needed — `services/treasury/` is the root of its own independent Cargo workspace.

```
services/treasury/
├── Cargo.toml              # workspace manifest
├── Cargo.lock
├── build.rs                # tonic-build proto codegen
├── proto/
│   └── treasury.proto      # gRPC service contract
├── migrations/
│   ├── 20260317000000_create_schema.sql
│   ├── 20260317000001_relay_logs.sql
│   ├── 20260317000002_pool_snapshots.sql
│   └── 20260317000003_watcher_alerts.sql
├── src/
│   ├── main.rs             # starts tonic server, runs startup validation
│   ├── proto.rs            # tonic::include_proto!("treasury") — generated types
│   ├── config.rs           # env-based config
│   ├── hot_path/
│   │   ├── mod.rs
│   │   ├── listener.rs     # stub: HotPathInitiated event listener
│   │   └── relayer.rs      # stub: releaseHotPath tx submission
│   ├── cold_path/
│   │   ├── mod.rs
│   │   └── rebalancer.rs   # stub: CCIP burn-and-mint
│   ├── watcher/
│   │   ├── mod.rs
│   │   └── verifier.rs     # stub: cross-reference source/dest events
│   ├── pool/
│   │   ├── mod.rs
│   │   └── manager.rs      # stub: pool depth tracking
│   └── db/
│       └── mod.rs          # sqlx pool setup, migration runner
├── docker-compose.yml      # postgres only
└── .env.example            # all required env vars documented
```

---

## 2. gRPC Contract (`proto/treasury.proto`)

```protobuf
syntax = "proto3";
package treasury;

service TreasuryService {
  rpc HealthCheck(HealthCheckRequest) returns (HealthCheckResponse);

  // Hot path (Task 05)
  rpc GetRelayStatus(GetRelayStatusRequest) returns (GetRelayStatusResponse);

  // Pool management (Task 05/10)
  rpc GetPoolDepth(GetPoolDepthRequest) returns (GetPoolDepthResponse);

  // Watcher (Task 06)
  rpc GetWatcherAlerts(GetWatcherAlertsRequest) returns (GetWatcherAlertsResponse);
}

message HealthCheckRequest {}
message HealthCheckResponse {
  enum Status { UNKNOWN = 0; SERVING = 1; NOT_SERVING = 2; }
  Status status = 1;
  bool db_connected = 2;
  bool rpc_reachable = 3;
  bool relayer_key_loaded = 4;
}

message GetRelayStatusRequest  { string source_event_hash = 1; }
message GetRelayStatusResponse { string status = 1; }

message GetPoolDepthRequest    { uint64 chain_id = 1; }
message GetPoolDepthResponse   { string depth_wei = 1; }

message GetWatcherAlertsRequest  { uint32 limit = 1; }
message GetWatcherAlertsResponse { repeated string alert_ids = 1; }
```

`build.rs` runs `tonic-build` to generate the server trait into `OUT_DIR`. `src/proto.rs` includes it via `tonic::include_proto!("treasury")`. Generated types are referenced throughout the codebase as `crate::proto::treasury_server::TreasuryServiceServer` and related types. All RPCs except `HealthCheck` return `Status::UNIMPLEMENTED` at scaffold stage. `HealthCheck` re-executes the three health checks live on each call (DB ping, RPC reachability, key file parse) and returns the aggregate result. This means the server can be running yet report `NOT_SERVING` if a downstream dependency becomes unavailable — useful for readiness probes.

**Phase 1 limitation:** `rpc_reachable` is a single `bool` — `true` only if all configured chains respond. Per-chain failure detail is not surfaced in the response; failures are logged server-side. Finer-grained chain-level reporting is out of scope for the scaffold.

---

## 3. Dependencies (`Cargo.toml`)

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime (`full` features) |
| `tonic` | gRPC server |
| `tonic-build` (build-dep) | Proto codegen |
| `prost` | Protobuf encoding |
| `sqlx` | Async PostgreSQL (`postgres`, `runtime-tokio-rustls`, `macros`, `migrate`) |
| `alloy` | `Address` type in `Config` (`features = ["primitives"]`); EVM interaction deferred to Tasks 05/06 |
| `tracing` | Structured logging |
| `tracing-subscriber` | Log formatting / env filter |
| `envy` | Env var → struct deserialization (scalar fields only) |
| `serde` | Serialization (for `envy` and JSON config fields) |
| `serde_json` | Deserialize JSON-encoded env vars for multi-chain maps |
| `reqwest` | Raw JSON-RPC calls for RPC reachability check (`features = ["json", "rustls-tls"]` — matches `sqlx`'s `runtime-tokio-rustls` TLS backend to avoid link conflicts) |
| `thiserror` | Error types |

---

## 4. Database Schema

Four migrations under `treasury` schema. All run via `sqlx::migrate!()` at startup.

### Migration 0: Schema creation
```sql
-- 20260317000000_create_schema.sql
CREATE SCHEMA IF NOT EXISTS treasury;
```

### `relay_logs`
```sql
-- 20260317000001_relay_logs.sql
CREATE TABLE treasury.relay_logs (
    id                BIGSERIAL PRIMARY KEY,
    source_event_hash TEXT        NOT NULL UNIQUE,
    dest_tx_hash      TEXT,
    source_chain_id   BIGINT      NOT NULL,
    dest_chain_id     BIGINT      NOT NULL,
    recipient         TEXT        NOT NULL,
    amount_wei        NUMERIC(78) NOT NULL,
    status            TEXT        NOT NULL DEFAULT 'pending',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### `pool_snapshots`
```sql
-- 20260317000002_pool_snapshots.sql
CREATE TABLE treasury.pool_snapshots (
    id          BIGSERIAL PRIMARY KEY,
    chain_id    BIGINT      NOT NULL,
    depth_wei   NUMERIC(78) NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### `watcher_alerts`
```sql
-- 20260317000003_watcher_alerts.sql
CREATE TABLE treasury.watcher_alerts (
    id                BIGSERIAL PRIMARY KEY,
    source_event_hash TEXT        NOT NULL,
    alert_type        TEXT        NOT NULL,
    detail            TEXT,
    resolved          BOOLEAN     NOT NULL DEFAULT false,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

`NUMERIC(78)` safely holds any `uint256` value. `status` is plain `TEXT` (values: `pending`, `released`, `failed`).

---

## 5. Configuration (`config.rs`)

Construction is two-step. All `Address` fields are parsed manually via `Address::from_str` (not via `envy`) to avoid relying on `alloy`'s serde impl interoperating correctly with `envy`'s string coercion path.

1. String/integer scalar fields are loaded via a `#[derive(serde::Deserialize)]` helper struct through `envy::from_env::<ScalarConfig>()`. `grpc_port` defaults to `50051` via `#[serde(default = "default_grpc_port")]` where `fn default_grpc_port() -> u16 { 50051 }`.
2. All other fields are read manually via `std::env::var(...)` and parsed explicitly, then assembled into `Config`:
   - `ROUTE_RECEIVER_ADDRESS` → `Address::from_str(...)?`
   - `RPC_URLS_JSON` → `serde_json::from_str::<HashMap<u64, String>>(...)?`
   - `CONTRACT_ADDRESSES_JSON` → `serde_json::from_str::<HashMap<u64, String>>(...)?` then each value parsed with `Address::from_str`

```rust
use alloy::primitives::Address;

// Internal helper — only plain string/int scalar fields
#[derive(serde::Deserialize)]
struct ScalarConfig {
    database_url: String,
    #[serde(default = "default_grpc_port")]
    grpc_port: u16,
    relayer_key_path: String,  // env: RELAYER_KEY_PATH
}

// Public config — fully constructed in Config::from_env()
pub struct Config {
    pub database_url: String,
    pub grpc_port: u16,
    pub rpc_urls: HashMap<u64, String>,            // env: RPC_URLS_JSON = {"1":"https://...","8453":"https://..."}
    pub contract_addresses: HashMap<u64, Address>, // env: CONTRACT_ADDRESSES_JSON = {"1":"0x...","8453":"0x..."}
    pub route_receiver_address: Address,           // env: ROUTE_RECEIVER_ADDRESS = "0x..."
    pub relayer_key_path: String,
}
```

`Address` is `alloy::primitives::Address` (enabled via `alloy` with `features = ["primitives"]`).

`.env.example` documents every variable with format examples, including JSON examples for `RPC_URLS_JSON` and `CONTRACT_ADDRESSES_JSON`.

---

## 6. Docker Compose

Postgres only. The service runs locally via `cargo run`.

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: web3bank
      POSTGRES_USER: treasury
      POSTGRES_PASSWORD: treasury
    ports: ["5432:5432"]
    volumes: ["treasury_pgdata:/var/lib/postgresql/data"]
volumes:
  treasury_pgdata:
```

---

## 7. Startup Validation

Startup proceeds in two sequential phases before the gRPC server binds:

**Phase A — Migrations (sequential, must complete before Phase B):**
Run `sqlx::migrate!("../migrations")` from `db/mod.rs` (path is relative to the file at compile time, resolving to `services/treasury/migrations/`). This acquires the Postgres advisory lock, applies all pending migrations, and releases the lock. If this fails, log the error and abort immediately. Migration failure is distinct from connectivity failure and is reported separately to ops.

**Phase B — Concurrent readiness checks (via `tokio::join!`):**
All three checks run to completion regardless of individual failures. After all three finish, if any failed: log each failure with context, then abort. This ensures the operator sees all missing dependencies at once.

1. **DB connectivity** — send `SELECT 1` to confirm the connection pool is live post-migration
2. **RPC reachability** — send `eth_blockNumber` via raw JSON-RPC (`reqwest`) to each configured chain URL; `rpc_reachable` is `true` only if **all** configured chains respond successfully
3. **Relayer key** — read file at `relayer_key_path`, parse as a `0x`-prefixed 64-character hex string (UTF-8, optional trailing newline); verify it decodes to a valid 32-byte value

`HealthCheck` RPC re-executes Phase B checks live on each call (not Phase A — migrations do not re-run). Returns `db_connected`, `rpc_reachable`, `relayer_key_loaded` individually. The aggregate `status` is `SERVING` only if all three pass, otherwise `NOT_SERVING`.

---

## 8. Acceptance Criteria

- `cargo build` compiles without errors
- `cargo test` passes, including:
  - Unit test: `Config` parses correctly from a set of env vars (including valid JSON for `RPC_URLS_JSON` and `CONTRACT_ADDRESSES_JSON`)
  - Integration test: all four migrations apply cleanly using `#[sqlx::test]` — the macro creates a temporary logical database on a running Postgres instance configured via `DATABASE_URL`. **A Postgres service must be available** — the macro does not launch Postgres itself. Local: `docker compose up postgres`. CI: add a Postgres service container and set `DATABASE_URL`.
- `docker compose up` brings up Postgres
- `HealthCheck` gRPC call returns a response (`SERVING` or `NOT_SERVING` depending on env)
- All non-`HealthCheck` RPCs return `UNIMPLEMENTED`

---

## 9. Out of Scope (Deferred to Later Tasks)

| Deferred | Task |
|----------|------|
| Hot path event listener logic | Task 05 |
| `releaseHotPath` tx submission | Task 05 |
| Pool depth tracking logic | Task 05 |
| Watcher cross-referencing logic | Task 06 |
| CCIP cold path rebalancing | Task 10 |
| HSM / threshold sig key management | Post-testnet |
