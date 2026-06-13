# Add Account Balance Aggregation and Complete Activity Feed

## Why

The product's core promise is "users see a single USD balance" — but no backend capability actually delivers it. Today `TreasuryService.GetBalance` returns `SUM(amount_wei)` over `treasury.relay_logs WHERE recipient = $1 AND status = 'completed'`, and the frontend dashboard polls this value every 10 seconds via the BFF `balance` query. The result is wrong for every primary flow:

- **Deposits are invisible.** Bob deposits $5,000 → dashboard still shows $0 (relay logs only record hot-path relays).
- **Outgoing sends are not subtracted.** Bob sends $1,000 → his displayed balance does not decrease.
- **Same-chain transfers are invisible** in both balance and activity (they are plain SyncUSD `transfer`s, never indexed).
- **Withdrawals are invisible.**
- **The activity feed shows only incoming hot-path relays** (`WHERE recipient = $1`) — a user never sees their own outgoing sends, deposits, or withdrawals.

The user's actual balance lives on-chain as SyncUSD `balanceOf(address)` — potentially spread across multiple chains (home-chain delivery means a user can hold SyncUSD on more than one chain). No spec covers this read path; this change defines it.

## What Changes

- **New capability `account-queries`**: defines what "balance" and "activity" mean for a user and which service computes them.
- **Balance** becomes the **sum of on-chain SyncUSD `balanceOf(address)` across all non-decommissioned chains**, served by Treasury (the only service allowed to touch chains), proxied by the BFF.
- **Activity feed** becomes a complete, indexed history: deposits, withdrawals, same-chain SyncUSD transfers, and hot-path transfers in **both directions** (sent and received).
- **Treasury gains a SyncUSD event indexer** (new `treasury.account_events` table) that ingests `Deposited`, `Withdrawn`, `Transfer`, `HotPathInitiated`, and `HotPathReleased` events per chain. The existing home-chain indexer (which already polls `Deposited` events but persists nothing) is subsumed by or layered on this index.
- **BREAKING (internal API)**: `GetBalance` semantics change from "sum of incoming relays" to "aggregated on-chain balance"; `GetRecentTransfers` is replaced or extended by an account-activity RPC returning typed entries (kind, direction, counterparty, chain, amount, status, tx hash).

## Capabilities

### New Capabilities

- `account-queries`: user-facing read model — aggregated cross-chain balance and complete account activity history; ownership (Treasury computes, BFF proxies, User Service never touches chains).

### Modified Capabilities

<!-- none — cross-chain-routing's relay audit trail and user-identity's home-chain rules are unchanged; this adds a read path on top -->

## Impact

- `services/treasury`: new event indexer module + `treasury.account_events` migration; `GetBalance` re-implemented as multi-chain `balanceOf` aggregation (RPC `eth_call`), new/changed activity RPC; home-chain indexer refactored to consume the shared event index.
- `packages/proto/treasury`: `GetBalance` doc semantics, new `GetAccountActivity` (or extended `GetRecentTransfers`) message types.
- `services/bff`: `balance` and `recentTransfers` resolvers map to the new semantics (schema shape can stay compatible for the frontend).
- `apps/bank-client`: no required changes (already polls `balance`/`recentTransfers`); display simply becomes correct.
- Unlocks: the decommission drain orchestrator's `HolderIndexPort` (needs exactly this holder/event index — currently has no production data source).
