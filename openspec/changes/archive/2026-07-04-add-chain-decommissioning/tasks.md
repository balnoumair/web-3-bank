# Tasks — Chain Decommissioning

## 1. On-chain — RouteReceiver (`packages/onchain`)

- [x] Add `decommissioned` flag per chain (separate from `active`/`inactive`).
- [x] Add admin-only `markDecommissioning(chainId)` (sets `draining` flag) and `finalizeDecommission(chainId)` (sets `decommissioned`, irreversible).
- [x] Update `getLatestRoute()` and consumer-facing reads to expose the third state.
- [x] Tests:
  - [x] `decommissioned` cannot be unset.
  - [x] CRE-driven `active`/`inactive` transitions cannot affect a `decommissioned` chain.

## 2. On-chain — Bank Contract (`packages/onchain`)

- [x] Add `freezeForDecommission()` — admin-only, one-shot. Sets a `frozen` flag.
- [x] In `frozen` state:
  - [x] `deposit`, `transferHotPath` (source), `releaseHotPath` (destination): revert.
  - [x] `withdraw`: still allowed.
  - [x] `rebalance` (cold path drain): still allowed.
  - [x] `bridgeReserve` (USDC drain): still allowed.
- [x] Add `pausePermanently()` — admin-only, called by governance after drain completes; uses existing `Pausable` but cannot be unpaused.
- [x] Tests for each frozen-state behavior.

## 3. Treasury — Drain Orchestrator (`services/treasury`)

- [x] Migration: create `treasury.decommission_ops` table.
  - Columns: chain_id, holder_address, amount, src_message_id, dst_chain_id, dst_tx_hash, status, started_at, completed_at, failure_reason.
- [x] New module `decommission.rs`:
  - [x] On `markDecommissioning` event: kick off drain plan for the target chain.
  - [x] Build holder set: query Treasury's existing event index for all Transfer events on the dying chain; reduce to current balances; cross-check each `balanceOf()` on-chain before processing.
  - [x] For each holder:
    - [x] Submit CCIP burn on dying chain → mint on target chain (via the existing cold-path `rebalance`, scoped per-holder if CCIP supports per-recipient mint, else batch into pool and credit user via separate Bank Contract function — TBD in implementation).
    - [x] Update `users.home_chain` via User Service `SetUserHomeChain(holder, target_chain_id)`.
    - [x] Record in `decommission_ops`.
  - [x] After all holders drained: drain pool via `rebalance`, drain reserve via `bridgeReserve`.
  - [x] On completion: signal governance (operator alert / PagerDuty) that finalization is ready.
- [x] Resumability: drain orchestrator is restartable; uses `decommission_ops` status to skip already-completed holders.
- [x] Tests:
  - [x] Drain happy path with mock holders.
  - [x] Drain target chain goes inactive mid-flight: pauses cleanly, alerts.
  - [x] Restart mid-drain: resumes from last completed holder.

## 4. User Service (`services/user-service`)

- [x] Add admin gRPC method `SetUserHomeChain(address, chain_id)` with auth restricted to Treasury's decommission orchestrator.
- [x] Audit log every call (operator action history).

## 5. CRE (`apps/cre-workflows`)

- [x] Exclude `decommissioned` chains from scoring entirely.
- [x] If CRE evaluates a `decommissioned` chain, skip silently (do not produce active/inactive output).

## 6. BFF & frontend

- [x] BFF: never route to a `decommissioned` chain (treat like inactive but permanent).
- [x] No UI changes — users do not see chains.

## 7. Governance & operations

- [x] Runbook: "How to decommission a chain"
  - [x] Pre-flight: select drain target chain, confirm reserves and pool depth on target are adequate.
  - [x] Step 1: governance multisig calls `freezeForDecommission` and `markDecommissioning`. **Grace period of 7 days begins.**
  - [x] Step 2: monitor drain progress via `decommission_ops` dashboard. If drain target becomes inactive mid-flight, drain auto-pauses; operators decide whether to wait or migrate to a new target.
  - [x] Step 3: after grace period elapses and drain completes, governance calls `pausePermanently` and `finalizeDecommission`.
- [x] Drain progress dashboard (Grafana or equivalent).
- [x] Document explicit non-coverage: Tempo cannot be decommissioned via this procedure.

## 8. Documentation

- [x] Add `docs/operations/chain-decommissioning.md` with the runbook.
- [x] Update `docs/user-journey.md` if the third chain state is product-relevant (likely not — invisible to users).
