# Service Architecture Specification

## Purpose

Define the structural invariants of the web3Bank backend: which service owns what, how services talk to each other, and what patterns smart contracts must follow. These are the rules every future change MUST respect.

## Requirements

### Requirement: Four backend services with strict boundaries

The backend SHALL consist of exactly four services, each owning a distinct domain:

| Service | Runtime | Owns |
|---|---|---|
| BFF | Bun + TypeScript | GraphQL boundary, JWT sessions |
| User Service | Rust | `users.*` schema, gRPC API |
| Treasury Service | Rust | `treasury.*` schema, hot/cold path, watcher |
| CRE Route Orchestrator | TypeScript on Bun | chain scoring, on-chain publishing |

#### Scenario: New backend behavior must find a home

- **WHEN** a change introduces new backend behavior
- **THEN** it SHALL be assigned to exactly one of the four services according to the ownership table
- **AND** no fifth service SHALL be introduced without a spec change to this requirement

### Requirement: BFF is a thin proxy

The BFF SHALL NOT own a database, SHALL NOT contain business logic, SHALL NOT listen for on-chain events, and SHALL NOT make routing decisions. Its responsibilities SHALL be limited to:

- Receiving GraphQL queries and mutations from the frontend
- Proxying requests to backend services
- Transforming backend responses into frontend-friendly shapes
- Managing JWT sessions for UI state (distinct from on-chain passkey signing)

#### Scenario: A feature needs persistence or chain access

- **WHEN** a proposed BFF feature requires a database, on-chain reads, or a routing decision
- **THEN** that logic SHALL be implemented in the owning backend service (User Service, Treasury, or CRE)
- **AND** the BFF SHALL only proxy the resulting API

### Requirement: Per-service schema isolation

Each service that uses PostgreSQL SHALL own its own schema and SHALL NEVER query another service's schema. The User Service owns `users.*`; the Treasury Service owns `treasury.*`. Cross-schema joins SHALL NOT exist.

#### Scenario: A service needs data owned by another

- **WHEN** a service needs data that lives in another service's schema
- **THEN** it SHALL request the data via the owning service's gRPC or HTTP API
- **AND** SHALL NOT issue a SQL query against the other schema

### Requirement: search_path enforced in pool setup

Each Rust service SHALL enforce its schema isolation via `after_connect` in the database pool setup, configuring `search_path` to only its owned schema.

#### Scenario: A pooled connection is opened

- **WHEN** a Rust service's pool opens a new database connection
- **THEN** the `after_connect` hook SHALL set `search_path` to the service's owned schema before the connection serves any query

### Requirement: Inter-service protocols are fixed

Inter-service communication SHALL use the following protocols:

| From | To | Protocol |
|---|---|---|
| Frontend | BFF | GraphQL over HTTPS |
| BFF | User Service | gRPC (port 50051) |
| BFF | Treasury Service | gRPC |
| Treasury | User Service | gRPC (home-chain updates) |
| Treasury | `RouteReceiver.sol` | On-chain reads (RPC) |
| Treasury | Bank Contracts | On-chain transactions (RPC) |
| CRE | `RouteReceiver.sol` | On-chain writes (via Chainlink DON) |

#### Scenario: A new inter-service call is added

- **WHEN** a change introduces communication between two services
- **THEN** it SHALL use the protocol listed for that pair
- **AND** introducing a new pair or protocol SHALL require updating this table via a spec change

### Requirement: Smart contracts use OpenZeppelin AccessControl

All contracts SHALL use OpenZeppelin `AccessControl` with the following roles:

| Role | Holder |
|---|---|
| `MINTER_ROLE` | Bank Contract and CCIP Token Pool |
| `RELAYER_ROLE` | Treasury Service relayer address |
| `ADMIN_ROLE` | Timelock contract |
| `PAUSER_ROLE` | Treasury Service watcher |

#### Scenario: Caller without the required role

- **WHEN** an address without the required role calls a role-gated function (e.g. mint without `MINTER_ROLE`)
- **THEN** the call SHALL revert with an AccessControl error and no state SHALL change

### Requirement: UUPS upgradeability with Timelock

All contracts SHALL use the UUPS upgradeable proxy pattern. All upgrades SHALL be gated behind `ADMIN_ROLE`, which SHALL be held by a Timelock contract enforcing a minimum delay before execution.

#### Scenario: Upgrade attempt bypassing the Timelock

- **WHEN** an address other than the Timelock attempts `upgradeToAndCall`
- **THEN** the upgrade SHALL revert
- **AND** an upgrade scheduled through the Timelock SHALL only execute after the minimum delay elapses

### Requirement: Emergency pause

All contracts SHALL inherit OpenZeppelin `Pausable`. When paused, all state-mutating functions (`deposit`, `withdraw`, `transferHotPath`, `releaseHotPath`) SHALL be disabled. Unpause SHALL require `ADMIN_ROLE` (and therefore the Timelock).

#### Scenario: Watcher pauses a Bank Contract

- **WHEN** the watcher calls `pause()` on a Bank Contract after detecting a mismatch
- **THEN** `deposit`, `withdraw`, `transferHotPath`, and `releaseHotPath` SHALL revert until unpaused
- **AND** unpausing SHALL require `ADMIN_ROLE` via the Timelock
