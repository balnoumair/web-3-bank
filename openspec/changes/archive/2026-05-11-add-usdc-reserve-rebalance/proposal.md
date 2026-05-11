# Add USDC Reserve Rebalance Across Chains

## Why

Each Bank Contract holds two distinct ledgers: a **SyncUSD pool** (used for hot-path releases) and a **USDC reserve** (the underlying backing that fulfills withdraws). The cold-path rebalance (`add-cold-path-rebalance`) keeps SyncUSD pools balanced. **It does not move USDC reserves.**

Reserves drift independently based on net deposit-vs-withdraw flow per chain. If users predominantly deposit on Tempo and withdraw on Base, Tempo's reserve overflows while Base's runs dry. Withdraws on Base then revert despite the system having ample total USDC.

This change introduces a mechanism to physically bridge USDC between Bank Contract reserves so withdrawals continue to work on every active chain. It is the precondition for true withdrawal failover described in `docs/user-journey.md` step 6.

## What changes

1. **Add `Bank.sol::bridgeReserve(uint64 destChainId, uint256 amount)`** — a permissioned function that bridges USDC from this chain's reserve to the destination chain's reserve, using a pluggable bridge adapter.
2. **New role `RESERVE_REBALANCER_ROLE`**, distinct from `REBALANCER_ROLE` and `RELAYER_ROLE`. Granted to a separate Treasury signer to limit blast radius.
3. **Pluggable bridge adapter interface** (`IReserveBridge`) so each chain can use its supported bridge:
   - Circle CCTP (Cross-Chain Transfer Protocol) on supported chains (Base, Arbitrum, etc.).
   - Custom adapter for chains where CCTP is unavailable (notably Tempo). Implementation per chain is out of scope for this proposal — the adapter contract is set per-chain by governance.
4. **Treasury reserve monitor** — Treasury reads `reserveDepth()` on each Bank Contract and triggers `bridgeReserve` when a chain falls below its target threshold.
5. **New audit table** `treasury.reserve_ops` with columns: source chain, destination chain, amount, bridge type, bridge-specific message id, status, started_at, completed_at, failure_reason.
6. **Refine `cross-chain-routing` spec** to describe reserve rebalance as a parallel concern to pool rebalance.

## Out of scope

- **Per-chain bridge adapter implementations.** This change defines the interface and the orchestration. Concrete adapters (CCTP wrapper, Tempo-specific bridge) are separate work tracked in tasks but not part of the spec deltas.
- **Withdrawal-time failover routing.** A user submitting a withdraw on a chain with a depleted reserve still gets a revert. Reserve rebalance prevents the depletion in steady state. Reactive cross-chain withdraw fulfillment (user submits on chain A, bank pays out on chain B) is a separate UX/contract problem deferred to a future change.
- **Multiple underlying stablecoins.** USDC only, matching the current product.
- **Rebalance to/from chains not on the active list.** Reserve bridging respects the same `RouteReceiver` activation gate as the cold path.

## Dependencies

- Independent of `add-cold-path-rebalance`. Can ship in either order.
- Independent of `add-home-chain-routing`.
- A precondition for `add-chain-decommissioning` — draining a chain's USDC reserve uses this mechanism.

## Impact

- `packages/onchain` — new function, new role, new adapter interface, governance-set adapter address per chain.
- `services/treasury` — new monitor module; new audit table migration; ABI bindings.
- No changes to user-service, BFF, frontend, CRE.
- Operational: per-chain bridge adapter contracts must be deployed and registered before this change becomes effective on each chain.
