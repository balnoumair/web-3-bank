# Tasks — Chain Decommissioning

## 1. On-chain — RouteReceiver (`packages/onchain`)

- [ ] Add `decommissioned` flag per chain (separate from `active`/`inactive`).
- [ ] Add admin-only `markDecommissioning(chainId)` (sets `draining` flag) and `finalizeDecommission(chainId)` (sets `decommissioned`, irreversible).
- [ ] Update `getLatestRoute()` and consumer-facing reads to expose the third state.
- [ ] Tests:
  - [ ] `decommissioned` cannot be unset.
  - [ ] CRE-driven `active`/`inactive` transitions cannot affect a `decommissioned` chain.

## 2. On-chain — Bank Contract (`packages/onchain`)

- [ ] Add `freezeForDecommission()` — admin-only, one-shot. Sets a `frozen` flag.
- [ ] In `frozen` state:
  - [ ] `deposit`, `transferHotPath` (source), `releaseHotPath` (destination): revert.
  - [ ] `withdraw`: still allowed.
  - [ ] `rebalance` (cold path drain): still allowed.
  - [ ] `bridgeReserve` (USDC drain): still allowed.
- [ ] Add `pausePermanently()` — admin-only, called by governance after drain completes; uses existing `Pausable` but cannot be unpaused.
- [ ] Tests for each frozen-state behavior.

## 3. Treasury — Drain Orchestrator (`services/treasury`)

- [ ] Migration: create `treasury.decommission_ops` table.
  - Columns: chain_id, holder_address, amount, src_message_id, dst_chain_id, dst_tx_hash, status, started_at, completed_at, failure_reason.
- [ ] New module `decommission.rs`:
  - [ ] On `markDecommissioning` event: kick off drain plan for the target chain.
  - [ ] Build holder set: query Treasury's existing event index for all Transfer events on the dying chain; reduce to current balances; cross-check each `balanceOf()` on-chain before processing.
  - [ ] For each holder:
    - [ ] Submit CCIP burn on dying chain → mint on target chain (via the existing cold-path `rebalance`, scoped per-holder if CCIP supports per-recipient mint, else batch into pool and credit user via separate Bank Contract function — TBD in implementation).
    - [ ] Update `users.home_chain` via User Service `SetUserHomeChain(holder, target_chain_id)`.
    - [ ] Record in `decommission_ops`.
  - [ ] After all holders drained: drain pool via `rebalance`, drain reserve via `bridgeReserve`.
  - [ ] On completion: signal governance (operator alert / PagerDuty) that finalization is ready.
- [ ] Resumability: drain orchestrator is restartable; uses `decommission_ops` status to skip already-completed holders.
- [ ] Tests:
  - [ ] Drain happy path with mock holders.
  - [ ] Drain target chain goes inactive mid-flight: pauses cleanly, alerts.
  - [ ] Restart mid-drain: resumes from last completed holder.

## 4. User Service (`services/user-service`)

- [ ] Add admin gRPC method `SetUserHomeChain(address, chain_id)` with auth restricted to Treasury's decommission orchestrator.
- [ ] Audit log every call (operator action history).

## 5. CRE (`apps/cre-workflows`)

- [ ] Exclude `decommissioned` chains from scoring entirely.
- [ ] If CRE evaluates a `decommissioned` chain, skip silently (do not produce active/inactive output).

## 6. BFF & frontend

- [ ] BFF: never route to a `decommissioned` chain (treat like inactive but permanent).
- [ ] No UI changes — users do not see chains.

## 7. Governance & operations

- [ ] Runbook: "How to decommission a chain"
  - [ ] Pre-flight: select drain target chain, confirm reserves and pool depth on target are adequate.
  - [ ] Step 1: governance multisig calls `freezeForDecommission` and `markDecommissioning`. **Grace period of 7 days begins.**
  - [ ] Step 2: monitor drain progress via `decommission_ops` dashboard. If drain target becomes inactive mid-flight, drain auto-pauses; operators decide whether to wait or migrate to a new target.
  - [ ] Step 3: after grace period elapses and drain completes, governance calls `pausePermanently` and `finalizeDecommission`.
- [ ] Drain progress dashboard (Grafana or equivalent).
- [ ] Document explicit non-coverage: Tempo cannot be decommissioned via this procedure.

## 8. Documentation

- [ ] Add `docs/operations/chain-decommissioning.md` with the runbook.
- [ ] Update `docs/user-journey.md` if the third chain state is product-relevant (likely not — invisible to users).
