# Add Chain Decommissioning Procedure

## Why

`add-cold-path-rebalance` and `add-usdc-reserve-rebalance` keep healthy chains balanced. `RouteReceiver`'s `inactive` flag handles transient outages — degraded chains are rerouted around until they recover.

But there is a third state with no current procedure: **a chain is permanently retired** (security incident, deprecated by its operator, planned migration off the chain, sustained failure with no recovery). Without a procedure:

- User SyncUSD on the dying chain is stranded indefinitely.
- USDC reserves on the dying chain are stuck.
- The system has no way to "shut down" a chain cleanly.

This is a real-money loss scenario. It needs an explicit, governance-triggered procedure — not an autonomous one — that drains user balances, drains reserves, and removes the chain from routing.

## What changes

1. **Introduce a third chain state: `decommissioned`** in `RouteReceiver.sol`, distinct from `active` and `inactive`. `decommissioned` is set only by governance and is one-way (no path back to `active`).
2. **Bank Contract `freezeForDecommission()`** — admin-only one-shot. Once called:
   - Rejects all new `deposit`, `transferHotPath` (as source or destination), and `releaseHotPath`.
   - Continues to allow `withdraw` during the grace period (so users who can act on the chain may exit themselves).
   - Continues to allow `bridgeReserve` and `rebalance` initiated by Treasury (drain operations).
3. **Treasury drain orchestrator** — a new module that, once a chain is frozen:
   - Enumerates SyncUSD holders on the dying chain (via Treasury's existing event indexer; see design.md for two implementation options).
   - For each holder, bridges their SyncUSD via CCIP burn-and-mint to a target healthy chain.
   - Updates `users.home_chain` for each affected user to the target chain via the User Service.
   - Drains the SyncUSD pool to the target chain (uses `rebalance`).
   - Drains the USDC reserve to the target chain (uses `bridgeReserve`).
   - Marks the drain complete; signals governance to publish `decommissioned` state in `RouteReceiver`.
4. **Treasury, BFF, and CRE** treat `decommissioned` as terminal: never route to the chain, never read its pool depths, never include it in scoring.
5. **Audit table `treasury.decommission_ops`** records every drain step (which holder, amount bridged, source tx, destination tx, status).

## Out of scope

- **Reactivating a decommissioned chain.** One-way. Reactivation, if ever desired, is a separate change.
- **User-side claim flow** (merkle drop). This proposal drains autonomously. A claim-based alternative is discussed in design.md but rejected for the default flow.
- **Cross-chain failure during drain** (e.g., the target healthy chain itself goes inactive mid-drain). Drain MUST be safely resumable; partial-drain handling is a tasks-level detail, not a spec change.
- **Decommissioning Tempo specifically.** Tempo is the system's home chain in many flows; decommissioning it is a higher-order migration that this procedure is not designed for. Document explicitly.

## Dependencies

- **Hard dependency on `add-cold-path-rebalance`** — drain uses CCIP burn-and-mint via `rebalance`.
- **Hard dependency on `add-usdc-reserve-rebalance`** — drain uses `bridgeReserve`.
- **Soft dependency on `add-home-chain-routing`** — drain updates `home_chain` for affected users; if home-chain-routing has not landed, the `home_chain` writes are no-ops but the user's balance still moves.

## Impact

- `packages/onchain` — adds `freezeForDecommission`, `decommissioned` flag in RouteReceiver, governance entry points.
- `services/treasury` — new drain orchestrator module; new audit table.
- `services/user-service` — adds bulk `SetUserHomeChain` admin path used only during decommission.
- `apps/cre-workflows` — CRE excludes decommissioned chains from scoring.
- Operational: governance runbook for executing a decommission. Significant — this procedure moves user funds.
