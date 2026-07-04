# Design — Chain Decommissioning

## Goals

- Provide a safe, governance-controlled path to permanently retire a chain.
- No user funds are stranded after a successful decommission.
- The procedure is auditable end-to-end and resumable on failure.

## Non-goals

- Decommission as an autonomous response to chain health. This is **always** a deliberate governance action, not a CRE-driven one. CRE produces `inactive`; only governance produces `decommissioned`.
- Reactivation. One-way operation.
- Decommissioning the system's anchor chain (Tempo). Out of scope.

## State machine

| State | Set by | Effects |
|---|---|---|
| `active` | CRE Orchestrator | All routes available. Hot path, cold path, deposits, withdraws, transfers all work. |
| `inactive` | CRE Orchestrator (auto, real-time) | Hot path and cold path skip this chain as source/destination. Existing balances on chain still accessible if chain itself is up. Recoverable when CRE re-marks active. |
| `decommissioned` | Governance only (manual) | Permanent. All routes refuse this chain. Bank Contract is frozen (no new ops except drain). After drain completes, contract is permanently paused. |

`decommissioned` is set in two phases:
1. **Freeze.** Governance calls `freezeForDecommission()` on the dying chain's Bank Contract and publishes a "draining" flag in RouteReceiver. Treasury starts the drain orchestrator. User-visible operations on the chain are blocked except direct `withdraw` (grace period).
2. **Finalize.** After Treasury reports drain complete, governance publishes `decommissioned` in RouteReceiver and `pause()`s the Bank Contract permanently.

## Key design decisions

### 1. Autonomous drain, not user-claim

Two approaches considered:

- **Autonomous drain (chosen):** Treasury enumerates holders and bridges everyone's SyncUSD without user action. User opens the app one day and their funds are now on a different chain (which they can't see anyway).
- **Claim-based:** publish a merkle root snapshot, users claim their balance on a healthy chain via UX action.

Chosen autonomous because:
- The product premise hides chains from users. Asking them to "claim from Arbitrum" exposes the abstraction we built to hide.
- Stale claims are a liability. Some users won't return for years; their funds sit unclaimed and operationally heavy.
- The mechanism is already trusted — the bank custodies via Bank Contract reserves. Moving SyncUSD between chains is well within the existing trust model.

Trade-off: significant gas cost for the bank. Each holder requires a separate CCIP message. Mitigation: batched CCIP calls if the protocol supports it; otherwise queue and pace the drain over days.

### 2. Holder enumeration via Treasury's event indexer

Two implementation paths:

- **(a) Treasury reads `Transfer` events** from contract genesis to derive the current holder set. Treasury already indexes events for the watcher. Adding a holder-set view is incremental.
- **(b) On-chain enumeration via `EnumerableSet`.** Bank Contract maintains the holder set in storage. Reliable but adds gas to every Transfer.

Choose **(a)** because option (b) imposes a permanent gas cost on every user transfer for a procedure that runs zero times in the happy path. Treasury already has the data.

Risk: Treasury's index could be incomplete (missed events, restored snapshot drift). Mitigation: drain orchestrator cross-checks each holder's claimed balance against `balanceOf()` on-chain immediately before bridging.

### 3. Withdraw remains open during grace period

After freeze, `withdraw()` still works on the dying chain. Why: users who can act may prefer to take USDC out themselves rather than have their SyncUSD bridged to a chain they don't expect. Grace period gives that option. After grace expires (governance-set, suggested 7 days), drain proceeds for whatever balances remain.

If the dying chain is fully offline, withdraw isn't possible anyway. Grace period only helps in slow-degradation scenarios.

### 4. Drain target is a single chosen healthy chain per decommission, not "spread across all"

Simpler accounting; one destination chain per drain operation. Governance picks the target as part of the decommission proposal. The cold path can rebalance the resulting concentration afterward.

### 5. `home_chain` reassignment happens during drain

For users whose `home_chain` was the decommissioned chain, the User Service updates `home_chain` to the drain target chain. This is the **only** permitted post-creation mutation of `home_chain` (per the `add-home-chain-routing` spec).

If `add-home-chain-routing` has not yet landed, this step is a no-op.

## Decisions

- **Grace period: 7 days.** Window between `freezeForDecommission` (withdrawals stay open, deposits/transfers blocked) and the start of automated drain. Gives users who can act on the chain time to exit themselves.
- **Drain target failure: manual pause-and-alert.** If the chosen target chain becomes inactive mid-drain, Treasury pauses the drain, marks in-flight ops, and pages operators. Governance decides whether to wait for target recovery or pick a new target. Auto-fallback to a different target was rejected because already-bridged users would land on a different chain than remaining holders, complicating accounting.
- **Holder-set sanity: balanceOf is authoritative.** If Treasury's event index disagrees with on-chain `balanceOf` for a holder, log the discrepancy and use `balanceOf` as the truth. The index is a convenience for enumeration; on-chain is the source of truth for the actual amount to bridge.

## Open questions

- **Bulk CCIP support.** If CCIP supports batched multi-recipient mint in a single message, drain gas drops dramatically. Treated as an implementation-time discovery rather than a spec decision: investigate when building the drain orchestrator. If unavailable, fall back to one message per holder and pace the drain over the grace period.
