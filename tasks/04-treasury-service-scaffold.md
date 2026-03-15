# Task 04: Treasury Service Scaffold

**Service:** `treasury-service` (Rust)
**Depends on:** None (can use mock contracts for initial development)
**Can parallelize with:** Task 01, Task 02, Task 05, Task 06, Task 07

## Goal

Set up the Rust project structure for the Treasury Service with module boundaries, database schema, and configuration.

## Scope

### Project Setup
- Initialize Rust project at `services/treasury/`
- Cargo workspace structure with clear modules
- Dependencies: `tokio`, `sqlx` (Postgres), `alloy` (EVM interaction), `tracing` (logging)
- Docker Compose for local PostgreSQL

### Module Structure
```
treasury/
├── src/
│   ├── main.rs
│   ├── config.rs            # Environment config, RPC URLs, contract addresses
│   ├── hot_path/
│   │   ├── mod.rs
│   │   ├── listener.rs      # Event listener for HotPathInitiated
│   │   └── relayer.rs       # Submits releaseHotPath transactions
│   ├── cold_path/
│   │   ├── mod.rs
│   │   └── rebalancer.rs    # CCIP burn-and-mint operations
│   ├── watcher/
│   │   ├── mod.rs
│   │   └── verifier.rs      # Cross-references source/dest events
│   ├── pool/
│   │   ├── mod.rs
│   │   └── manager.rs       # Pool depth tracking, threshold checks
│   └── db/
│       ├── mod.rs
│       └── migrations/       # sqlx migrations
```

### Database Schema (`treasury` schema)
- `relay_logs`: records every hot path relay (source_event_hash, dest_tx_hash, amount, status, timestamps)
- `pool_snapshots`: periodic pool depth recordings per chain
- `watcher_alerts`: any detected mismatches

### Configuration
- Environment-based config: RPC URLs per chain, contract addresses, relayer key path, Postgres URL
- `RouteReceiver.sol` address for chain health reads

### Health Check & Startup
- HTTP health check endpoint
- Startup validation: DB connectivity, RPC reachability, relayer key loaded

## Acceptance Criteria
- `cargo build` compiles without errors
- `cargo test` passes (unit tests for config, DB migrations)
- Docker Compose brings up Postgres + service locally
- Health check endpoint responds
