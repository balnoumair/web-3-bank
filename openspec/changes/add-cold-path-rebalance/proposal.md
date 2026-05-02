# Add Cold Path Pool Rebalance

## Why

The cold path is described in `docs/user-journey.md` (step 5) and partially specified in `cross-chain-routing/spec.md`, but it is not executable end-to-end. The Treasury Service has the orchestration code (`services/treasury/src/cold_path.rs`), but the on-chain entry point — `Bank.sol::rebalance()` — does not exist. Every cold-path operation the Treasury submits today reverts.

Without the cold path, SyncUSD pool depths drift indefinitely under cross-chain hot-path traffic. Eventually destination pools starve and the hot path itself begins to fail.

## What changes

1. **Add `Bank.sol::rebalance(uint64 destChainId, uint256 amount)`** as a permissioned entry point that initiates a CCIP burn-and-mint of SyncUSD from the local pool to the destination chain's pool.
2. **Restrict invocation** to a new `REBALANCER_ROLE` granted only to Treasury Service signers. Distinct from `RELAYER_ROLE` (hot path).
3. **Wire CCIP burn-and-mint** through Chainlink's `BurnMintTokenPool`, already a dependency in `packages/onchain`.
4. **Refine `cross-chain-routing` spec** to make the cold path's contract surface explicit, including idempotency, per-call cap, and audit trail.
5. **Connect Treasury cold-path code** to the new contract function. The call site already exists in `cold_path.rs::execute_rebalance`; it just needs the ABI to match.

## Out of scope

- **USDC reserve rebalancing across chains.** This change rebalances the *SyncUSD pool* (which fulfills hot-path releases). It does **not** move *USDC reserves* (which back deposits and fulfill withdraws). USDC reserve drift is a separate concern requiring physical USDC bridging (e.g., Circle CCTP) and will be a future change.
- **Withdrawal failover.** Cross-chain withdrawal fulfillment depends on USDC reserve handling and is therefore deferred to the change that addresses reserves.
- **Chain decommissioning.** Permanent chain exclusion and the user-balance migration that requires is a separate concern. Will be a future change.
- **Receive-on-home-chain routing.** Send-time routing to a recipient's home chain is a separate change (`add-home-chain-routing`).
- **User balance pre-positioning.** Cold path here only touches pool-owned liquidity. Autonomous movement of user-owned SyncUSD is not introduced.
- **Rebalance batching.** The existing `cross-chain-routing` spec mentions batching for gas efficiency. Treated as a follow-up if it complicates the initial implementation.

## Impact

- `packages/onchain` — adds Solidity function, role, CCIP wiring on Bank Contract.
- `services/treasury` — minor: regenerate ABI bindings, error handling for new revert paths.
- No changes to user-service, BFF, frontend, or CRE orchestrator.
- Database: no migration. Existing `treasury.rebalance_ops` table already has the right shape; insert will record the CCIP `messageId`.
