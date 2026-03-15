# Bank Infrastructure & Cross-Chain Routing

> **Status:** 🚧 WIP — Core concepts defined. Service boundaries and hot path authorization model decided.

## Vision

web3Bank operates as a chain-abstracted, non-custodial banking ledger where the user's funds are represented by a custom stablecoin (`SyncUSD`).

The user's experience is entirely chain-agnostic. They see a single USD balance, while the system transparently manages the distribution, security, and routing of those funds across multiple blockchains (with Tempo as the primary hub).

## Core Mechanisms

### 1. The Custom Stablecoin (`SyncUSD`)
Instead of an off-chain database ledger, the user's balance *is* their token balance. 
- The bank issues `SyncUSD` (a custom stablecoin backed 1:1 by deposits like USDC).
- On **Tempo**, this is a native **TIP-20** token (yielding benefits like payment lanes, stable fees, and native reconciliation memos).
- On overflow chains (e.g., Base, Arbitrum), it is a standard ERC-20 token.

### 2. Chain Abstraction (The "Pool" Model)
Users do not choose which chain they transact on. The system handles routing behind the scenes.
- **Hot Path (Instant Transfers):** When User A sends funds to User B, if they are on different chains, we perform a **pool-to-pool swap**. The bank contract on Chain A debits User A, and the bank contract on Chain B credits User B from its local liquidity pool. CCIP is *not* used in the hot path, ensuring transfers are instant and cheap.
- **Cold Path (Asynchronous Rebalancing):** The treasury maintains pools of `SyncUSD` on all active chains. When a pool becomes imbalanced, the backend system triggers a batched, asynchronous CCIP burn-and-mint operation to move liquidity between chains. 

### 3. The Role of the CRE Load Balancer
The `cre-route-orchestrator` (an existing TypeScript/Bun service built on the Chainlink CRE SDK) does not route individual A→B transfers. Instead, it manages the global health of the bank by publishing chain scores and activation states to the `RouteReceiver.sol` smart contract on-chain:
1. **Dynamic Reserve Distribution:** It scores chains (Fee 35%, Latency 30%, Reliability 25%, Liquidity 10%) and publishes the optimal ranking to `RouteReceiver.sol`.
2. **Activation State:** Chains scoring above the activation threshold (default 0.7) are marked as active. The Treasury Service reads this on-chain state to decide where to route funds.
3. **Failover:** If a chain becomes degraded, CRE drops its score and publishes an updated activation state. The Treasury Service reads the updated state and stops routing to that chain.

### 4. The Role of the Treasury Service
The Treasury Service (Rust) is the operational backbone that moves funds. It reads chain health from `RouteReceiver.sol` and acts accordingly:
1. **Hot Path Relay:** Listens for `HotPathInitiated` events on source chains, validates the destination chain is active (via `RouteReceiver.sol`), checks pool depth, and submits release transactions on the destination chain.
2. **Cold Path Rebalancing:** Executes batched CCIP burn-and-mint operations when pools become imbalanced.
3. **Watcher:** An independent module that cross-references every hot path release against the source chain event. On mismatch, it pauses the affected Bank Contract.
4. **Pool Depth Management:** Monitors pool depths across all chains and rejects hot path transfers when the destination pool is insufficient.

### 5. Pool Depth & Liquidity Safeguards
- **Minimum Pool Ratio:** Each chain must hold a configurable minimum percentage of the total SyncUSD supply.
- **Hot Path Rejection:** If the destination pool's available liquidity is below the transfer amount, the hot path transfer is rejected. The user receives an error and can retry when liquidity is available.
- **Rebalancing Triggers:** When a pool drops below its target threshold, the Treasury Service initiates a cold path CCIP rebalance proactively — before the pool is fully depleted.

## Concrete User Flows & Examples

To understand how this operates in practice, it helps to compare it to the traditional banking system. When you Venmo a friend, the money doesn't actually move between bank reserves in real-time. Instead, the banks update local ledgers instantly (the **Hot Path**) and settle the actual funds in bulk at the end of the day (the **Cold Path**). 

web3Bank uses the exact same model, but built entirely on-chain using smart contracts and CCIP.

### 1. User Onboarding & Deposit
**Scenario:** Bob signs up and wants to deposit $5,000.
1. Bob creates an account using **Tempo Native Passkeys**. Behind the scenes, a domain-bound EIP-2718 account is created for him on the Tempo blockchain.
2. Bob deposits 5,000 USDC into the web3Bank Smart Contract on Tempo.
3. The Bank Contract locks the USDC in its reserve pool and mints **5,000 `SyncUSD`** directly to Bob's Tempo account.
4. **UX:** Bob's dashboard simply says: *Balance: $5,000.00*.

### 2. Same-Chain Transfer (The Happy Path)
**Scenario:** Bob (on Tempo) sends $500 to Alice (also on Tempo).
1. Bob initiates a transfer from the web3Bank UI.
2. Under the hood, this is a standard **TIP-20 token transfer** of 500 `SyncUSD` from Bob's address to Alice's address on Tempo.
3. Since it's on Tempo, the fee is sponsored by the bank (~$0.001) and settles within ~0.5 seconds.
4. **UX:** Alice instantly sees her balance increase by $500.

### 3. Cross-Chain Transfer (The Hot Path)
**Scenario:** Bob (on Tempo) sends $1,000 to Charlie (wallet connected to Base).
Because CCIP takes 5–20 minutes, we **never** use it in the hot path for user transfers.
1. Bob initiates the transfer of $1,000 from the UI.
2. **Tempo Bank Contract:** Takes 1,000 `SyncUSD` from Bob and holds it in the Tempo pool. Emits a `HotPathInitiated` event.
3. **Treasury Service (Relayer):** Listens for the event, reads `RouteReceiver.sol` to confirm Base is an active chain, verifies the Base pool has sufficient depth, and submits a release transaction on Base.
4. **Base Bank Contract:** Releases 1,000 `SyncUSD` from its *local liquidity pool* to Charlie's wallet on Base (caller must have `RELAYER_ROLE`).
5. **Treasury Service (Watcher):** Independently verifies the release matches the source event. If any mismatch is detected, the watcher triggers the Bank Contract's `pause()` mechanism.
6. **UX:** Charlie receives the funds instantly. Bob's balance updates instantly. No one waits for CCIP.

### 4. Background Settlement (The Cold Path)
**Scenario:** After thousands of cross-chain transfers, the Base liquidity pool is running low.
1. The `cre-route-orchestrator` constantly monitors pool depths across all chains.
2. It detects that the Base pool is $50,000 below its target threshold, while the Tempo pool has a $50,000 surplus.
3. The backend Treasury Service triggers a **Rebalance Operation**.
4. **Action:** The system burns 50,000 `SyncUSD` from the Tempo pool, sends a **CCIP message**, and mints 50,000 `SyncUSD` into the Base pool.
5. **UX:** This happens asynchronously in the background. Users are completely unaffected and unaware of the 15-minute settlement time.

### 5. Chain Failover (Resilience)
**Scenario:** The Arbitrum network experiences a multi-hour outage.
1. The `cre-route-orchestrator` detects the degradation and drops Arbitrum's health score to 0.
2. The Arbitrum pool is immediately removed from the active routing set.
3. New deposits are automatically routed to Tempo or Base.
4. If a user requests a withdrawal of funds they originally deposited on Arbitrum, the Treasury Service transparently fulfills the withdrawal by releasing USDC from the healthy Base or Tempo pools instead.
5. **UX:** The bank remains 100% operational despite the underlying chain outage.

---
*Last updated: March 2026*
