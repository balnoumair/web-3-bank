# Design — Cold Path Pool Rebalance

## Goals

- The cold path executes successfully end-to-end without any user-visible effect.
- Rebalances are atomic in intent (one operation = one CCIP message), idempotent (safe to retry), and audited (every attempt logged regardless of outcome).
- Pool drift below a configurable target ratio triggers a rebalance proactively, before hot-path releases on the deficit chain begin to fail.

## Non-goals

- USDC reserve management. The cold path moves SyncUSD pool liquidity, not the underlying USDC backing.
- Cold path running without CRE chain-health input. The rebalance MUST consult `RouteReceiver.sol` for the destination chain's activation state — same gate as the hot path.
- On-chain triggering policy. The contract exposes the function; Treasury decides when to call it.

## Two ledgers per Bank Contract — clarifying terminology

Each Bank Contract holds two distinct balances:

| Ledger | What it holds | Moves on |
|---|---|---|
| **USDC reserve** | Underlying USDC backing | Deposits (in) and withdrawals (out) |
| **SyncUSD pool** | SyncUSD owned by the contract, used to fulfill cross-chain hot-path releases | `transferHotPath` (in), `releaseHotPath` (out), and **`rebalance` (this change)** |

This change touches **only the SyncUSD pool**. Cross-chain USDC reserve rebalancing is a separate problem with a different mechanism (Circle CCTP or equivalent) and is out of scope.

## Key design decisions

### 1. Burn on source, mint on destination via CCIP

CCIP `BurnMintTokenPool` is already chosen at the system level (per `service-architecture` spec and existing `packages/onchain` dependencies). Alternatives considered:

- **Lock-and-mint (escrow on source):** rejected. Doubles supply temporarily, complicates accounting, and CCIP burn-and-mint is the supported path for our token model.
- **Off-chain accounting only:** rejected. Violates the "no off-chain ledger" principle in `banking-ledger/spec.md`.

### 2. `REBALANCER_ROLE`, distinct from `RELAYER_ROLE`

Separation of concerns. The relayer (hot path) and rebalancer (cold path) operate on different timescales and failure modes. A compromise of one role should not enable the other. Each role is granted to a separate Treasury signer.

### 3. Triggering policy lives in Treasury, not on-chain

The Bank Contract exposes the function but does not decide when to call it. Treasury reads pool depths via `poolDepth()`, evaluates against the configured target ratio, and submits the rebalance.

This keeps the contract minimal and lets the policy evolve without contract upgrades.

### 4. Idempotency via CCIP `messageId`

Each `rebalance()` call produces a CCIP message with a unique `messageId`. The destination Bank Contract's `_ccipReceive` handler MUST reject any `messageId` it has already processed. The `treasury.rebalance_ops` audit table keys on `messageId` for the same reason.

### 5. Per-rebalance cap on-chain

The Bank Contract enforces a maximum amount per `rebalance` call (`maxRebalanceAmount`, configurable via admin/governance). This bounds blast radius if `REBALANCER_ROLE` is compromised: an attacker can drain at most one cap-sized chunk per source-chain block before the watcher and governance can intervene.

Treasury already caps per-operation amounts in `cold_path.rs`; the on-chain cap is defense-in-depth.

### 6. CCIP destination allowlist

The Bank Contract maintains an allowlist of permitted destination chain IDs for outbound CCIP messages, and an allowlist of permitted source contracts for inbound CCIP messages. This protects against accidental or malicious cross-chain wiring and is standard CCIP integration practice.

## Initial parameters (decided)

These values are runtime configuration, not part of the spec deltas. They are recorded here so implementation has a concrete starting point. Governance may revise post-launch.

- **`maxRebalanceAmount`:** 5% of total SyncUSD supply, set per chain via governance multisig. Reviewed periodically.
- **Stuck-CCIP-message handling: manual.** Treasury times out an in-flight rebalance after a configurable window (initial: 30 minutes — CCIP typical settlement is ~15 min, so 2× headroom), marks the op `failed` in `treasury.rebalance_ops`, and alerts operators. Auto-retry is **not** in scope for this change. Implementation may add it later only if it is trivially safe — i.e., re-issuing a new CCIP message with a new `messageId` and no risk of double-mint, given the destination's `processedMessages` idempotency guard.
- **Pool target threshold: equal-share with 80% floor.** Each active chain's target SyncUSD pool depth is `total_pool_supply / num_active_chains`. Treasury triggers a rebalance *from* a chain whose pool exceeds 100% of its target, *to* any chain whose pool falls below 80% of its target. Both percentages are Treasury-side configuration; the defaults here apply at launch.

## Open questions

None remaining for this change.
