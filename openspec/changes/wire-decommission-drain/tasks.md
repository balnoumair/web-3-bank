# Tasks — Wire the Decommission Drain

## 1. Prerequisite

- [ ] 1.1 Confirm `add-account-balance-and-activity` is implemented (needs `treasury.account_events` + `index_cursors`); rebase on it.

## 2. Port adapters (`services/treasury`)

- [ ] 2.1 `PgHolderIndexRepository` implementing `HolderIndexPort`: distinct source-chain holders from `account_events`; `balance_of` via live `eth_call`.
- [ ] 2.2 `ChainStatePort` adapter reusing the RouteReceiver read behind `IsChainActive`.
- [ ] 2.3 `BankDrainPort` adapter: holder bridge, `rebalance`, `bridgeReserve` submissions via `eth/tx.rs` with the relayer/rebalancer key; map receipts into `BridgeReceipt`.
- [ ] 2.4 User-service update adapter: `SetUserHomeChain` with `decommission_override=true` and orchestrator token.

## 3. Trigger and status RPCs

- [ ] 3.1 Proto: `StartDecommissionDrain(source_chain, target_chain) → drain_id`, `GetDecommissionDrainStatus(drain_id)`; regenerate stubs.
- [ ] 3.2 Token gate via request metadata (`DECOMMISSION_ADMIN_TOKEN`); reject missing/invalid tokens.
- [ ] 3.3 Precondition checks: source frozen+draining, target active, roles present, index cursor within head tolerance, no drain already running.
- [ ] 3.4 Plan builder: holders from index, pool amount from pool depth, reserve amount from reserve depth; spawn `run_drain_plan` as background task.
- [ ] 3.5 Residual-supply invariant check between holder drain and pool/reserve drain; pause + watcher-style alert on violation.
- [ ] 3.6 Status RPC aggregating `decommission_ops` (running / paused / resumable / completed).

## 4. Wiring and cleanup

- [ ] 4.1 `main.rs`: construct orchestrator with real adapters; remove the unused `_decommission_repo` binding.
- [ ] 4.2 Config: `DECOMMISSION_ADMIN_TOKEN`, index head tolerance, dust tolerance — add to `.env.example` (tell the user what to set; do not touch `.env`).

## 5. Tests and rehearsal

- [ ] 5.1 Unit: precondition rejections (not frozen, inactive target, stale index, concurrent drain, bad token).
- [ ] 5.2 Unit: plan building from a seeded `account_events` fixture, zero-balance holders skipped.
- [ ] 5.3 Integration (two-chain anvil): freeze A → drain to B → kill treasury mid-drain → resume via re-invocation → verify `decommission_ops`, final balances, and home-chain updates.
- [ ] 5.4 Update `docs/operations/chain-decommissioning.md` Step 2 with the concrete `grpcurl` trigger + status commands and a "resume after deploy" note.
