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
**Purpose:** The single entry point for the frontend to communicate with the broader web3Bank system. It orchestrates requests, acts as a secure proxy, and manages off-chain user metadata (e.g., matching a passkey credential to an internal profile) without containing downstream business logic.
- **Runtime:** **Bun** 
- **Rationale:** Bun provides extreme performance, built-in TypeScript support out-of-the-box, an integrated native test runner, and high-throughput networking capabilities.

## 3. Core Backend Services
**Purpose:** Houses the complex and high-stakes business logic, including the **Treasury Service** (monitoring cross-chain liquidity and executing CCIP pool swaps) and the **CRE Route Orchestrator** (dynamically scoring blockchains for health, fees, and liquidity).
- **Language:** **Rust**
- **Rationale:** Rust delivers memory safety, fearless concurrency, and maximum performance without a garbage collector. These traits are strictly necessary for services handling real-time financial routing and multi-chain state synchronization.

---
*Last updated: March 2026*
