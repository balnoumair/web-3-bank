## Context

The Treasury Service keeps USDC reserves balanced across chains via `bridgeReserve` (CCTP), tracked operationally in `treasury.reserve_ops` — a state machine (`pending → submitted → relayed → completed | failed`) full of tx hashes, attestation bytes, and message IDs. That table answers "what is the status of bridge op X?" It does **not** answer two accounting questions:

1. **Where is reserve value right now?** Between a bridge's source burn and destination mint (CCTP attestation latency, typically minutes), the USDC has left the source chain and not arrived on the destination. Summing on-chain `reserveDepth()` across chains during that window under-counts the total.
2. **Does our view still match the chains?** There is no continuous proof that Treasury's understanding of reserves agrees with on-chain reality.

The chain is — and remains — the source of truth for reserves (`reserveDepth()` is authoritative, enforced by the Bank Contracts). This design adds a thin, internal, double-entry **mirror** that models reserve value (including in-flight) and reconciles itself against that on-chain truth. It lives entirely in the existing `treasury.*` Postgres schema.

## Goals / Non-Goals

**Goals:**
- Represent every chain's reserve, plus money in transit, as double-entry accounts whose books always balance.
- Make "how much USDC is mid-bridge right now?" a single query.
- Continuously reconcile ledger balances against on-chain `reserveDepth()` and alert on divergence beyond tolerance.
- Reuse the existing reserve-path lifecycle and watcher/alert machinery — no new infrastructure.
- Keep the behavioral contract backend-agnostic, so the ledger could move to a dedicated ledger DB later without changing requirements.

**Non-Goals:**
- **Not a source of truth.** The ledger never gates, blocks, or authorizes a bridge; it only observes and reconciles. If the ledger and chain disagree, the chain wins and the ledger raises an alert.
- **Not TigerBeetle (or any new database).** At a few ops/day the value is the accounting *model*, not throughput. A dedicated ledger DB is explicitly deferred.
- **No on-chain or contract changes.** Read-only use of `reserveDepth()`.
- **Not a user-facing ledger.** This is internal treasury reserve accounting, unrelated to user SyncUSD balances (which stay on-chain per `banking-ledger`).
- **Not replacing `reserve_ops`.** The operational state machine stays; the ledger is derived from its transitions.

## Decisions

### 1. Double-entry model: one account per chain + a single in-transit account

Accounts:
- `reserve:<chain_id>` — mirrors that chain's Bank Contract USDC reserve.
- `in_transit` — a single shared account holding value that has left a source chain but not yet landed.

Every bridge becomes two balanced transfers:

```
initiation  (reserve_ops → submitted):   debit reserve:<src>   credit in_transit
completion  (reserve_ops → completed):   debit in_transit      credit reserve:<dst>
```

At all times: `Σ reserve:<chain> + in_transit == total reserves`. The in-flight question is just `balance(in_transit)`.

**Alternative considered — net per-chain balances in a single row each (no transfers):** simpler, but loses the audit trail and the always-balanced invariant, and can't represent in-transit cleanly. Rejected; double-entry is the whole point.

### 2. Ledger is derived from `reserve_ops` transitions, not written independently

The ledger transfer is recorded in the **same database transaction** as the `reserve_ops` status update that triggers it (`submitted` for initiation, `completed` for completion). Both tables are in `treasury.*` Postgres, so this is a local ACID transaction — no distributed-commit problem. This guarantees the ledger can never drift from `reserve_ops` due to a partial write.

**Alternative considered — an async projector that tails `reserve_ops`:** more decoupled, but introduces lag and its own reconciliation problem (ledger vs. reserve_ops, on top of ledger vs. chain). Rejected as needless complexity at this scale.

### 3. Idempotent, append-only transfers keyed on `(op_id, leg)`

Transfers are immutable rows. Each carries `(op_id, leg)` where `leg ∈ {initiation, completion}`, with a unique constraint. Re-processing a lifecycle transition (retries, restart mid-loop) is a no-op insert. This mirrors how `reserve_ops` already tolerates re-entry, and matches the idempotency the reserve bridge itself requires (`messageId` replay protection).

### 4. Reconciliation as a separate periodic check, alerting only

A reconciliation loop (cadence configurable, e.g. alongside the existing watcher) reads `reserveDepth()` for each active chain and compares to `balance(reserve:<chain>)`. If `| ledger − chain | > tolerance`, it raises a watcher alert (reusing the existing alert table/path). It does **not** auto-correct the ledger — divergence means a bug or an unobserved on-chain event that a human should see. Tolerance accommodates the expected in-flight skew and rounding.

**Alternative considered — auto-heal the ledger to match the chain:** hides exactly the bugs reconciliation exists to surface. Rejected; alert, don't paper over.

### 5. Amounts as NUMERIC(78,0), consistent with `reserve_ops`

Reuse the existing `amount_wei NUMERIC(78,0)` representation already used across `treasury.*` for U256 wei values. No floats, ever.

## Risks / Trade-offs

- **[In-transit account never drains if a bridge fails (`reserve_ops → failed`).]** → A failed bridge must post a compensating transfer. If a bridge fails *after* the source debit, record a reversal `debit in_transit → credit reserve:<src>` (funds returned to source per CCTP semantics) or route to a dedicated `loss`/`stuck` account if truly lost. The completion leg fires only on observed on-chain completion, so the books reflect reality; the `failed` path MUST be specified so `in_transit` cannot leak. Covered as a requirement.
- **[Ledger and chain legitimately diverge during in-flight windows.]** → Reconciliation tolerance + comparing `reserve:<chain> + (in-transit attributable to that chain)` rather than the bare reserve account during active bridges. Keep the check simple: alert only on divergence that persists beyond a bridge's expected completion time.
- **[Ledger mistaken for source of truth by a future reader.]** → Documented as a hard non-goal in both proposal and spec; the ledger exposes no API that moves funds. Naming (`reserve_ledger`, "mirror") reinforces intent.
- **[Bootstrapping existing reserves.]** → On first deploy the ledger is empty while chains already hold reserves. Seed reserve accounts from a one-time `reserveDepth()` read (an opening-balance transfer from a `genesis` account), then let lifecycle transitions take over. Covered in Migration Plan.

## Migration Plan

1. Add migration creating `treasury.reserve_ledger_accounts` and `treasury.reserve_ledger_transfers` (or a single transfers table with derived balances) in the `treasury.*` schema.
2. Backfill: read `reserveDepth()` for each active chain once and post opening-balance transfers (`genesis → reserve:<chain>`) so the ledger starts reconciled.
3. Hook ledger writes into the existing `submitted`/`completed`/`failed` transitions in `reserve_path.rs`, in the same DB transaction.
4. Enable the reconciliation loop in observe-only mode (alerts only) — no behavior depends on it, so rollout is safe.
5. **Rollback:** the ledger is read-only with respect to funds and routing; disabling the reconciliation loop and dropping the tables removes the feature with zero impact on reserve operations.

## Open Questions

- Reconciliation cadence and tolerance values — pin during implementation against observed CCTP completion times.
- Single transfers table with computed balances vs. a materialized per-account balance column — a tasks-level decision; spec stays agnostic.
- Whether to attribute `in_transit` to source vs. destination during reconciliation, or keep it as one global pool (simpler, chosen unless reconciliation noise demands attribution).
