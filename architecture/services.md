# Service Architecture

> **Status:** 🚧 WIP — Service boundaries defined. Implementation pending.

## Overview

web3Bank is composed of four services with strict boundaries. Each service owns its domain and communicates through well-defined interfaces. The BFF never contains business logic or owns a database — it is a thin proxy that transforms and forwards requests for the frontend.

```
┌─────────────┐
│   Frontend   │  SolidJS + wagmi/viem
│ (bank-client)│  Tempo Native Passkeys
└──────┬───────┘
       │ GraphQL
       ▼
┌─────────────┐
│     BFF      │  Bun — Thin proxy, JWT sessions
│              │  No DB, no business logic
└──┬───────┬───┘
   │       │  Internal API calls
   ▼       ▼
┌──────┐ ┌──────────────┐
│ User │ │   Treasury    │  Rust — Hot path relay, cold path
│ Svc  │ │   Service     │  rebalancing, watcher, pool mgmt
└──────┘ └──────┬────────┘
                │ Reads on-chain
                ▼
         ┌──────────────┐
         │RouteReceiver │  Solidity — On-chain chain scores
         │   .sol       │  & activation states
         └──────┬───────┘
                │ Published by
                ▼
         ┌──────────────┐
         │     CRE      │  TypeScript/Bun — Chainlink CRE SDK
         │ Orchestrator │  Chain scoring, monitoring, failover
         └──────────────┘
```

## 1. BFF (Backend For Frontend)

| | |
|---|---|
| **Runtime** | Bun |
| **Protocol** | GraphQL (facing frontend) |
| **Database** | None |
| **Business Logic** | None |

The BFF is the frontend's single entry point to the backend. It:
- Receives GraphQL queries/mutations from the frontend
- Proxies requests to the appropriate backend service (User Service, Treasury Service)
- Transforms backend responses into frontend-friendly shapes
- Manages JWT sessions for UI state (distinct from on-chain passkey auth)

The BFF does **not**:
- Own a database
- Listen for on-chain events
- Make routing decisions
- Store user profiles or credentials
- Execute any business logic

## 2. Treasury Service

| | |
|---|---|
| **Runtime** | Rust |
| **Database** | PostgreSQL (`treasury` schema) |
| **On-Chain** | Reads `RouteReceiver.sol`, submits txs to Bank Contracts |

The Treasury Service is the operational backbone — it moves funds across chains. It is a single Rust binary with four clear modules:

### Hot Path Relay Module
- Listens for `HotPathInitiated` events on all active chains
- Reads `RouteReceiver.sol` to verify the destination chain is active
- Checks destination pool depth is sufficient for the transfer amount
- Submits `releaseHotPath()` transaction on the destination chain using its `RELAYER_ROLE` key
- Records the relay in PostgreSQL for audit trail

### Cold Path Rebalancing Module
- Monitors pool depths across all chains
- When a pool drops below its target threshold, initiates a CCIP burn-and-mint operation
- Batches multiple rebalancing operations for gas efficiency
- Reads target ratios from `RouteReceiver.sol` activation state

### Watcher Module
- Independently monitors all `HotPathReleased` events on destination chains
- Cross-references each release against the corresponding `HotPathInitiated` event on the source chain
- On any mismatch (amount, recipient, missing source event), triggers the Bank Contract's `pause()` mechanism
- Logs all verifications and alerts to PostgreSQL

### Pool Management Module
- Tracks real-time pool depths across all chains
- Maintains minimum pool thresholds (configurable per chain)
- Rejects hot path transfers when destination pool has insufficient liquidity
- Provides pool depth data to the BFF for frontend display

### Relayer Key Management
- The Treasury Service holds the private key(s) with `RELAYER_ROLE` on all Bank Contracts
- For testnet: single EOA managed by the service
- For production: threshold signature scheme or HSM-backed key management (deferred)

## 3. CRE Route Orchestrator

| | |
|---|---|
| **Runtime** | TypeScript/Bun (Chainlink CRE SDK) |
| **Database** | None (on-chain state via `RouteReceiver.sol`) |
| **Location** | `apps/cre-workflows/`, `apps/cre-runtime/`, `packages/cre-config/` |

The CRE Orchestrator scores blockchain health and publishes results on-chain. It does **not** route individual transfers or interact with the Treasury Service directly.

### What It Does
- **Scores chains** on 4 metrics: Fee (35%), Latency (30%), Reliability (25%), Liquidity (10%)
- **Fetches data** via Chainlink DON consensus: gas prices (RPC), block freshness (RPC), TVL (DeFiLlama)
- **Publishes** ranked chains and activation states to `RouteReceiver.sol`
- **Runs on a cron** (every 5 minutes) with an HTTP trigger for on-demand evaluation
- **Simulates** transactions via Tenderly before on-chain publication

### Integration with Treasury
The integration is entirely on-chain — no direct service-to-service communication:
1. CRE publishes chain scores and activation state to `RouteReceiver.sol`
2. Treasury reads `RouteReceiver.sol` to determine which chains are active
3. Treasury uses activation state to accept/reject hot path transfers to specific chains
4. Treasury uses chain rankings to inform cold path rebalancing priorities

### Current State
- Scoring engine: complete and spec-locked
- Chain monitoring: live on 5 testnet chains (Base, Arbitrum, Optimism, Polygon, Ethereum Sepolia)
- On-chain publishing: functional with replay protection
- Dashboard: SolidStart operator UI for monitoring runs
- `RouteReceiver.sol`: deployed on Base Sepolia

## 4. User Service

| | |
|---|---|
| **Runtime** | Rust |
| **Protocol** | gRPC (tonic, port 50051) |
| **Database** | PostgreSQL (`users` schema) |

The User Service manages user identity and account state:
- User profile CRUD (display name, preferences)
- Passkey credential-to-address mapping
- Account state (active, suspended)

This service is called by the BFF when the frontend needs user data. It does not interact with the blockchain directly.

## Data Layer

### PostgreSQL
- Single Postgres instance for testnet
- Each service owns its own schema: `treasury.*`, `users.*`
- Migrations managed per-service
- Rust services use `sqlx` (compile-time verified queries, async, connection pooling)

### On-Chain State
- `RouteReceiver.sol`: chain scores, activation states (owned by CRE)
- Bank Contracts: pool balances, user balances (SyncUSD)
- SyncUSD: token balances per chain

## Inter-Service Communication

| From | To | Protocol |
|------|----|----------|
| Frontend → BFF | GraphQL over HTTPS |
| BFF → User Service | gRPC (port 50051) |
| BFF → Treasury Service | Internal HTTP API |
| Treasury → RouteReceiver.sol | On-chain reads (RPC) |
| Treasury → Bank Contracts | On-chain transactions (RPC) |
| CRE → RouteReceiver.sol | On-chain writes (via Chainlink DON) |

The BFF communicates with backend services via internal HTTP APIs. There is no direct communication between Treasury and CRE — they are decoupled through on-chain state.

---
*Last updated: March 2026*
