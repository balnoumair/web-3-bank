# Add Withdrawal Failover for Users on Unhealthy Chains

## Why

`docs/user-journey.md` §6 promises: *"If a user on Arbitrum wants to withdraw: Treasury fulfills from Tempo or Base pool instead."* No spec requirement and no code backs this. Today `withdraw()` is strictly local — it burns SyncUSD and releases USDC **on the same chain** — so a user whose SyncUSD sits on an inactive chain simply cannot withdraw until the chain recovers, and the frontend has no signal to route around it. The failover promise also hides a hard problem the journey glosses over: a withdrawal needs the user's *signed* burn on the chain where their tokens live; if that chain can't process transactions at all, no service can move the user's funds non-custodially. This change decides and specs what failover actually means, so the docs, the specs, and the code stop disagreeing.

## What Changes

- **Decide the failover semantics** (design.md). Proposed: *withdrawal-by-routing*, not custodial fulfillment —
  - If the user's chain is **degraded but processing transactions**, withdrawal proceeds locally (no change).
  - If the user holds SyncUSD on multiple chains, the client withdraws on a healthy chain where balance exists (needs per-chain balance breakdown from `account-queries`).
  - If the user's only balance is on an **inactive** chain, the system SHALL be honest: the BFF/Treasury expose "withdrawal temporarily unavailable" status instead of pretending. Funds become withdrawable again when the chain recovers or is decommissioned (drain moves them to a healthy chain).
- **BFF withdrawal-routing query**: before building a `withdraw()` transaction, the client asks the BFF where withdrawal is possible (chain id + available amount per chain), mirroring the existing send-routing resolution pattern.
- **Correct `docs/user-journey.md` §6** to match the decided behavior — the current text implies custodial cross-chain fulfillment, which contradicts the non-custodial model.

## Capabilities

### Modified Capabilities

- `banking-ledger`: the withdraw requirement gains the multi-chain/health dimension (where a withdrawal may be executed; honest unavailability).
- `cross-chain-routing`: new withdrawal-routing resolution requirement (BFF resolves the withdrawal chain like it resolves send routing).

## Impact

- `services/bff` + `packages/proto/treasury`: withdrawal-routing query backed by per-chain balances and chain activation state (both exist after `add-account-balance-and-activity`).
- `apps/bank-client`: withdraw flow consults routing before building the transaction.
- `docs/user-journey.md`: §6 rewritten to the decided semantics.
- No contract changes (withdraw stays local and non-custodial).
- **Dependency**: `add-account-balance-and-activity` (per-chain balance breakdown).
