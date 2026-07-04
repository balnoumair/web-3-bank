# Wire the Decommission Drain Orchestrator into Production

## Why

`add-chain-decommissioning` (all tasks checked, merged in PR #38) delivered the on-chain pieces, the audit table, and a `decommission.rs` orchestrator — but the orchestrator is **dead code in production**. In `main.rs` the repository is bound to `_decommission_repo` and never used; `run_drain_plan` and `handle_mark_decommissioning` are called only from tests; every port (`HolderIndexPort`, `BankDrainPort`, `ChainStatePort`, user-service update) has only test doubles, and there is **no entry point** an operator could use to start a drain. Worse, the runbook says the orchestrator "enumerates SyncUSD holders from Treasury's indexed transfer/deposit history" — but no such history exists: `relay_logs` records hot-path relays only, and the home-chain indexer persists nothing. If governance froze a chain today, the documented procedure could not be executed. This is the gap between "the change's tasks are done" and "the capability works" — it must close before `add-chain-decommissioning` is archived or a chain ever needs retiring.

## What Changes

- **Concrete adapters for every orchestrator port**:
  - `HolderIndexPort` → backed by `treasury.account_events` (from `add-account-balance-and-activity`): enumerate addresses with nonzero indexed SyncUSD on the source chain, cross-checked against on-chain `balanceOf` (as the runbook already prescribes).
  - `BankDrainPort` → real CCIP `rebalance` / `bridgeReserve` / holder-bridge transaction submission via the existing `eth/tx.rs` signing path.
  - `ChainStatePort` → the same RouteReceiver read used by `IsChainActive`.
  - User-service home-chain updates → existing gRPC client with `decommission_override=true` and the orchestrator token.
- **Operator trigger**: an admin-only `StartDecommissionDrain(source_chain, target_chain)` gRPC method on the Treasury server (token-gated, mirrors the runbook's "Treasury starts the decommission orchestrator"), plus `GetDecommissionDrainStatus` for monitoring. Idempotent: re-invocation resumes from `decommission_ops`.
- **Wiring in `main.rs`**: construct the orchestrator with real adapters; remove the `_decommission_repo` dead binding.
- **Runbook update**: replace the hand-wavy "Treasury starts the orchestrator" with the concrete RPC invocation and status checks.

## Capabilities

### Modified Capabilities

- `cross-chain-routing`: the drain-procedure requirement gains the operator-trigger and holder-enumeration-source requirements (how a drain is started, resumed, and observed — currently unspecified).

## Impact

- `services/treasury`: adapters in `db/` + `eth/`, two new gRPC methods, `main.rs` wiring; depends on the `account_events` index.
- `packages/proto/treasury`: `StartDecommissionDrain`, `GetDecommissionDrainStatus`.
- `docs/operations/chain-decommissioning.md`: concrete trigger/monitor commands.
- **Dependency**: hard dependency on `add-account-balance-and-activity` (holder enumeration has no data source without it).
- **Sequencing note**: `add-chain-decommissioning` should not be archived as "done" until this lands, or should be archived with this change opened immediately as its successor.
