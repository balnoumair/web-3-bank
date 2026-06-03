## Why

The Treasury Service moves USDC reserves between chains with `bridgeReserve` (CCTP), but during the minutes a bridge is in flight the funds have **left the source chain and not yet arrived on the destination**. Summing on-chain `reserveDepth()` across chains during that window under-counts total reserves — the money looks like it briefly vanished. Treasury has no internal model that can answer "how much USDC is mid-bridge right now?" or continuously prove that its view of reserves still agrees with the chains.

This change adds a small **double-entry accounting ledger** of reserves, in Treasury's existing Postgres, whose only jobs are to (1) make in-flight reserves explicit so the books always balance, and (2) act as a reconciliation cross-check against on-chain truth.

## What Changes

- **New `treasury.reserve_ledger` model** (double-entry): a set of accounts and immutable balanced transfers recorded in Treasury's Postgres schema.
  - One **reserve account per chain** (mirrors that chain's Bank Contract USDC reserve).
  - One shared **in-transit account** holding value that has left a source chain but not yet landed on a destination.
- **Ledger entries are derived from the existing `reserve_ops` lifecycle**, not a new source of truth:
  - On bridge **initiation** (`reserve_ops` → `submitted`): record a transfer `debit source-chain reserve → credit in-transit`.
  - On bridge **completion** (`reserve_ops` → `completed`): record a transfer `debit in-transit → credit dest-chain reserve`.
- **Reconciliation check**: a periodic task compares each chain's ledger reserve-account balance against the chain's on-chain `reserveDepth()`. A divergence beyond a configured tolerance raises a watcher alert.
- **The ledger is explicitly a secondary mirror, NOT authoritative.** The chain remains the source of truth. The ledger never moves real funds and never gates a bridge; it only models and reconciles. It complements `treasury.reserve_ops` (the operational state machine) — it does not replace it.
- **Postgres-backed double-entry, not TigerBeetle.** At a few ops/day the value is the accounting model, not throughput; a dedicated ledger database is out of scope. The behavioral spec is written so the backend could be swapped later without changing requirements.

## Capabilities

### New Capabilities
- `reserve-accounting`: An internal, double-entry ledger of Treasury's cross-chain USDC reserves. Defines reserve and in-transit accounts, the balanced transfers derived from reserve-bridge lifecycle events, the always-balanced and never-negative-reserve invariants, and the reconciliation check against on-chain `reserveDepth()`.

### Modified Capabilities
<!-- None. The banking-ledger (user-facing SyncUSD) and cross-chain-routing (reserve bridge mechanics) specs are unchanged; this change only observes their effects and records them in a new internal ledger. -->

## Impact

- `services/treasury` — new `treasury.reserve_ledger` table(s) and migration; a `ReserveLedgerRepository` (driven port) + Postgres adapter; ledger writes hooked into the existing reserve-path lifecycle transitions in `reserve_path.rs`; a reconciliation loop (alongside the existing watcher) that reads `reserveDepth()` and compares to ledger balances; reuse of the existing watcher alert path for divergence alerts.
- **No on-chain / contract changes.** Reads `reserveDepth()` only (already used by the reserve planner).
- **No changes to other services or schemas.** Stays entirely within the `treasury.*` schema; no cross-schema access.
- Relates to: `banking-ledger` spec (reserve-backing model it mirrors) and the reserve rebalance flow (`reserve_ops`, `IReserveBridge` / CCTP).
