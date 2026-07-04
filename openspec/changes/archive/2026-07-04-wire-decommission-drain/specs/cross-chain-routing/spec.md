# Cross-Chain Routing — Delta

> Note: builds on the drain-procedure requirements introduced by `add-chain-decommissioning` (not yet archived). These are additive requirements covering how a drain is started, fed, and observed.

## ADDED Requirements

### Requirement: Drain is started by an authenticated operator call

The Treasury Service SHALL expose an admin-only `StartDecommissionDrain(source_chain, target_chain)` entry point gated by an operator token. Before starting, Treasury SHALL validate that the source chain is frozen and marked draining in RouteReceiver, that the target chain is active, and that Treasury holds the required Bank roles on the source chain. The call SHALL be idempotent with respect to resumption: re-invocation for a partially completed drain SHALL resume from the audit table rather than restart.

#### Scenario: Operator starts a drain on a frozen chain

- **WHEN** an operator with a valid token calls `StartDecommissionDrain` for a frozen, draining source chain and an active target chain
- **THEN** Treasury SHALL build the drain plan and execute the drain in the background, returning a drain identifier

#### Scenario: Trigger rejected when preconditions fail

- **WHEN** the source chain is not frozen, the target chain is not active, or the token is missing or invalid
- **THEN** Treasury SHALL reject the call without submitting any transaction

#### Scenario: Only one drain at a time

- **WHEN** `StartDecommissionDrain` is called while another drain is running
- **THEN** Treasury SHALL reject the call

### Requirement: Holder enumeration is grounded in the account event index and cross-checked on-chain

The drain plan's holder set SHALL be enumerated from Treasury's persistent account event index for the source chain, and every holder's amount SHALL be cross-checked against on-chain `balanceOf` before bridging. Treasury SHALL refuse to build a drain plan while the source chain's index cursor lags the chain head beyond a configured tolerance. After all holder bridges complete, Treasury SHALL verify that the remaining non-pool SyncUSD supply on the source chain is zero (within dust tolerance) before draining pool and reserve; on violation it SHALL pause and alert operators.

#### Scenario: Stale index blocks the drain

- **WHEN** the account event index for the source chain is behind the chain head beyond tolerance
- **THEN** `StartDecommissionDrain` SHALL be rejected until the index catches up

#### Scenario: Index over-enumeration is harmless

- **WHEN** the index proposes a holder whose live on-chain balance is zero
- **THEN** the orchestrator SHALL skip the holder without submitting a bridge

#### Scenario: Residual supply halts the drain

- **WHEN** holder bridging completes but non-pool SyncUSD supply on the source chain remains above dust tolerance
- **THEN** Treasury SHALL pause before pool and reserve drain and alert operators

### Requirement: Drain progress is observable

The Treasury Service SHALL expose a status query aggregating `treasury.decommission_ops` for a drain: per-status operation counts, drained amounts, and the most recent error. The status SHALL distinguish running, paused (target inactive or invariant violation), resumable (interrupted), and completed states.

#### Scenario: Operator monitors a running drain

- **WHEN** an operator queries drain status during execution
- **THEN** Treasury SHALL return per-status counts and amounts consistent with the audit table
