# Design — Wiring the Decommission Drain

## Context

`decommission.rs` is a well-tested control-flow core expressed against four ports (holder index, bank drain, chain state, user-service update) — hexagonal by the book, but with no production adapters and no driver. The supporting infrastructure exists elsewhere in the service: RPC + tx signing (`eth/`), RouteReceiver reads (server `IsChainActive`), the user-service gRPC client (`user_pb`), and the `decommission_ops` repository. The missing holder data source is being built by `add-account-balance-and-activity` (`treasury.account_events`).

## Goals / Non-Goals

**Goals:**
- An operator can start, resume, and observe a drain exactly as the runbook describes, with no code changes on the day.
- Holder enumeration is grounded in indexed events and cross-checked on-chain before any bridge.
- The orchestrator core stays untouched — this change only adds adapters and a driver.

**Non-Goals:**
- Changing drain semantics (ordering, resumability, target-inactive pause) — already specced and tested.
- Autonomous drain triggering. Governance/operator initiation only, per the original proposal.
- Decommissioning Tempo (explicitly out of scope in the parent change).

## Decisions

### 1. Trigger is a token-gated gRPC method, not a CLI binary
`StartDecommissionDrain(source, target)` on the existing Treasury server, gated by a bearer token in metadata (`DECOMMISSION_ADMIN_TOKEN`), mirroring how user-service gates `SetUserHomeChain` with `DECOMMISSION_ORCHESTRATOR_TOKEN`. The call validates preconditions (source frozen/draining in RouteReceiver, target active, roles present), builds the `DrainPlan`, and spawns the drain as a background task; it returns immediately with a drain id.

*Alternative:* separate admin CLI binary. Rejected — a second binary needs the same config/key plumbing, and the gRPC server already has health/monitoring conventions; `grpcurl` from the runbook is enough of a CLI.

### 2. Holder enumeration: index proposes, chain disposes
`PgHolderIndexRepository.holders_for_chain` returns distinct addresses with any indexed inbound SyncUSD activity on the source chain; `balance_of` does a live `eth_call`. The orchestrator already bridges only nonzero cross-checked balances, so over-enumeration from the index is harmless (zero balances are skipped) and under-enumeration is the real risk — mitigated by a completeness check: indexed cursor must be at the chain head (within a tolerance) before the plan is built, otherwise the trigger refuses to start.

### 3. One drain at a time
A drain is global state. The trigger rejects `StartDecommissionDrain` while another drain is running (in-process mutex + `decommission_ops` status check on startup so a crashed drain shows as resumable, not running).

### 4. Status surface reads the audit table only
`GetDecommissionDrainStatus` aggregates `decommission_ops` (per-status counts, amounts, last error) — the same queries the Grafana runbook panels use. No extra state to keep consistent.

## Risks / Trade-offs

- [Index incompleteness → stranded holder funds] → head-proximity precondition + on-chain `balanceOf` cross-check + final invariant: after holder drain, the source chain's total SyncUSD supply minus pool balance must be ~0 before pool/reserve drain proceeds; otherwise pause and alert.
- [Token-gated RPC is weaker than mTLS] → acceptable at testnet scope; matches the existing user-service pattern; noted for the deferred security/ops hardening item.
- [Long-running background task vs. deploys] → resumability already handles restart; the runbook gains a "resume after deploy" note.

## Migration Plan

1. Land after (or rebased on) `add-account-balance-and-activity`.
2. Adapters + wiring + RPCs; runbook update in the same PR.
3. Rehearse on a local two-chain anvil setup: freeze chain A, drain to chain B, kill treasury mid-drain, resume, verify `decommission_ops` and final balances.

## Open Questions

- Should `finalizeDecommission` publication remain a manual governance step after drain completion (current runbook) or be signaled by the status RPC? (Default: stays manual.)
