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

The service lives at `services/treasury/` inside the monorepo root as a standalone Cargo workspace. It is not part of the pnpm workspace.

```
services/treasury/
├── Cargo.toml              # workspace manifest
├── Cargo.lock
├── build.rs                # tonic-build proto codegen
├── proto/
│   └── treasury.proto      # gRPC service contract
├── migrations/
│   ├── 20260317000001_relay_logs.sql
│   ├── 20260317000002_pool_snapshots.sql
│   └── 20260317000003_watcher_alerts.sql
├── src/
│   ├── main.rs             # starts tonic server, runs startup validation
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

`build.rs` runs `tonic-build` to generate the server trait. All RPCs except `HealthCheck` return `Status::UNIMPLEMENTED` at scaffold stage. `HealthCheck` runs the real startup validation checks and returns live status.

---

## 3. Dependencies (`Cargo.toml`)

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime (`full` features) |
| `tonic` | gRPC server |
| `tonic-build` (build-dep) | Proto codegen |
| `prost` | Protobuf encoding |
| `sqlx` | Async PostgreSQL (`postgres`, `runtime-tokio-rustls`, `macros`, `migrate`) |
| `alloy` | EVM interaction (event listening, tx submission, contract reads) |
| `tracing` | Structured logging |
| `tracing-subscriber` | Log formatting / env filter |
| `envy` | Env var → struct deserialization |
| `serde` | Serialization (for `envy`) |
| `tokio` | Async runtime |
| `thiserror` | Error types |

---

## 4. Database Schema

Three migrations under `treasury` schema. All run via `sqlx::migrate!()` at startup.

### `relay_logs`
```sql
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
CREATE TABLE treasury.pool_snapshots (
    id          BIGSERIAL PRIMARY KEY,
    chain_id    BIGINT      NOT NULL,
    depth_wei   NUMERIC(78) NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### `watcher_alerts`
```sql
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

Loaded from environment variables at startup via `envy`. The service panics fast on missing/invalid config rather than failing at first use.

```rust
pub struct Config {
    pub database_url: String,
    pub grpc_port: u16,                            // default: 50051
    pub rpc_urls: HashMap<u64, String>,            // chain_id -> RPC URL
    pub contract_addresses: HashMap<u64, Address>, // chain_id -> BankContract address
    pub route_receiver_address: Address,           // RouteReceiver.sol (Base Sepolia)
    pub relayer_key_path: String,                  // path to relayer private key file
}
```

`.env.example` documents every variable with descriptions.

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

Before the gRPC server binds, three checks run concurrently via `tokio::join!`:

1. **DB** — run `sqlx::migrate!()` then `SELECT 1` to confirm connectivity
2. **RPC** — `eth_blockNumber` on each configured chain RPC URL
3. **Relayer key** — read file at `relayer_key_path`, parse as a valid private key

Any failure logs a clear error and aborts startup. The `HealthCheck` RPC reports live state of all three checks.

---

## 8. Acceptance Criteria

- `cargo build` compiles without errors
- `cargo test` passes (unit tests for config parsing, migration smoke test)
- `docker compose up` brings up Postgres
- `HealthCheck` gRPC call returns a response (SERVING or NOT_SERVING depending on env)
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
