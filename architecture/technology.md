# Technology Stack

> **Status:** 🚧 WIP — Core technologies defined. Additional packages will be listed as they are adopted.

## Core Paradigms

Across the entire web3Bank ecosystem, we design and build adhering strictly to two foundational software engineering paradigms:
- **Domain-Driven Design (DDD):** We structure the project around the business domain. The core logic (such as managing the Bank Ledger, computing the CRE chain scores, and Treasury Rebalancing) is strictly isolated from infrastructure and delivery mechanisms (APIs, Web3 RPCs).
- **Functional Programming (FP):** We rely on functional programming principles (pure functions, immutability, avoidance of shared mutable state), which is especially critical when dealing with financial transactions, blockchain state, and concurrent cross-chain routing.

## 1. Frontend Client
**Purpose:** Provides the user interface, managing native passkey authentication and Tempo network interactions.
- **Framework:** **SolidJS** via **SolidStart** (for optimized routing and server-side rendering).
- **Key Packages:**
  - `@wagmi/solid`: For managing blockchain state, connection hooks, and specific WebAuthn/passkey connectors native to Tempo.
  - `viem`: The lightweight underlying TypeScript interface used by wagmi for interacting with EVM networks and handling the low-level EIP-2718 passkey payloads.
  - `@tanstack/solid-query`: Used alongside wagmi for highly performant caching and deduplication of blockchain data.

## 2. Backend For Frontend (BFF)
**Purpose:** A thin GraphQL proxy — the frontend's single entry point to the backend. It forwards requests to internal services, transforms responses for the frontend, and manages JWT sessions. It does **not** own a database, store user profiles, or contain any business logic.
- **Runtime:** **Bun**
- **Rationale:** Bun provides built-in TypeScript support, an integrated test runner, and high-throughput networking — ideal for a lightweight proxy layer.

## 3. Treasury Service
**Purpose:** The operational backbone that moves funds across chains. Handles hot path relay (listening for on-chain events, submitting release transactions), cold path CCIP rebalancing, pool depth management, and the watcher (fraud detection via event cross-referencing).
- **Language:** **Rust**
- **Rationale:** Rust delivers memory safety, fearless concurrency, and maximum performance without a garbage collector. These traits are strictly necessary for a service handling real-time financial relay, multi-chain event processing, and latency-critical pool management.
- **Key Packages:**
  - `sqlx`: Compile-time verified async PostgreSQL queries.
  - `ethers-rs` / `alloy`: EVM interaction (event listening, transaction submission, contract reads).
  - `tokio`: Async runtime for concurrent chain monitoring.

## 4. CRE Route Orchestrator
**Purpose:** Scores blockchain health and publishes chain rankings and activation states to the `RouteReceiver.sol` smart contract. The Treasury Service reads this on-chain state to make routing decisions — there is no direct communication between the two services.
- **Language:** **TypeScript** (on **Bun** runtime)
- **Location:** `apps/cre-workflows/`, `apps/cre-runtime/`, `packages/cre-config/`
- **Rationale:** The CRE is tightly coupled to the **Chainlink CRE SDK** (JS/TS-native, runs as WASM in the Chainlink DON). Rewriting in Rust would lose SDK compatibility.
- **Key Packages:**
  - `@chainlink/cre-sdk`: CRE workflow runtime (DON consensus, cron triggers).
  - `zod`: Schema validation and type inference.

## 5. Smart Contracts
**Purpose:** On-chain logic for SyncUSD token, Bank Contract (liquidity pools), and RouteReceiver (chain health state).
- **Language:** **Solidity** 0.8.24+
- **Toolchain:** **Foundry** (Forge for compilation/testing, Cast for interaction, Anvil for local devnet).
- **Key Libraries:**
  - **OpenZeppelin Contracts**: `AccessControl`, `UUPSUpgradeable`, `Pausable`, `ERC20`.
  - **Chainlink CCIP**: `BurnMintERC20` extensions for cross-chain token transfers.

## 6. Data Layer
**Purpose:** Persistent storage for backend services that need it.
- **Database:** **PostgreSQL**
- **Rationale:** ACID guarantees for financial data, rich querying for audit trails, battle-tested at scale. Each service owns its own schema (`treasury.*`, `users.*`). Single instance for testnet, separate instances for production.
- The BFF does **not** have a database — it proxies to backend services.

---
*Last updated: March 2026*
