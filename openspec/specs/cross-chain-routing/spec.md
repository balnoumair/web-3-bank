# Cross-Chain Routing Specification

## Purpose

Make user transfers across chains feel instant by using local pool liquidity on the destination chain (**Hot Path**), and rebalance pool liquidity asynchronously via CCIP burn-and-mint (**Cold Path**). Users never wait for cross-chain settlement.
## Requirements
### Requirement: Hot Path uses local pool liquidity, not CCIP

A cross-chain user transfer SHALL be executed by debiting the source chain's pool and crediting the recipient from the destination chain's pool. CCIP SHALL NOT be used in the hot path.

#### Scenario: Bob on Tempo sends $1,000 to Charlie on Base

- **WHEN** Bob calls `transferHotPath(Charlie, 1000, BaseChainId)` on the Tempo Bank Contract
- **THEN** 1,000 `SyncUSD` SHALL be pulled from Bob and locked in the Tempo pool
- **AND** the contract SHALL emit a `HotPathInitiated` event
- **AND** the Treasury Service SHALL submit a `releaseHotPath()` transaction on Base
- **AND** the Base Bank Contract SHALL release 1,000 `SyncUSD` from its local pool to Charlie
- **AND** the Base Bank Contract SHALL emit a `HotPathReleased` event

### Requirement: Hot Path release is restricted and idempotent

`releaseHotPath` on a destination Bank Contract SHALL be callable only by an address holding `RELAYER_ROLE`. Each `sourceEventHash` SHALL be releasable at most once.

#### Scenario: Unauthorized caller invokes releaseHotPath

- **WHEN** an address without `RELAYER_ROLE` calls `releaseHotPath` on a destination Bank Contract
- **THEN** the call SHALL revert

#### Scenario: Replayed sourceEventHash

- **WHEN** `releaseHotPath` is called twice with the same `sourceEventHash`
- **THEN** the first call SHALL succeed
- **AND** the second call SHALL revert without releasing additional liquidity

### Requirement: Treasury validates destination before release

Before submitting a release transaction, the Treasury Service SHALL read `RouteReceiver.sol` and verify the destination chain is in the active set, and SHALL verify that the destination pool depth is greater than or equal to the transfer amount.

#### Scenario: Destination pool has insufficient liquidity

- **WHEN** the Treasury Service detects that the destination pool depth is less than the requested transfer amount
- **THEN** the hot path transfer SHALL be rejected
- **AND** the user SHALL receive an error and MAY retry later

#### Scenario: Destination chain is inactive

- **WHEN** the destination chain's activation state in `RouteReceiver.sol` is inactive
- **THEN** the hot path transfer SHALL be rejected

### Requirement: Watcher cross-references every release

The Treasury Service watcher module SHALL independently observe every `HotPathReleased` event on every chain and verify it corresponds to a matching `HotPathInitiated` event on the source chain (matching amount, matching recipient, source event present).

#### Scenario: Watcher detects a mismatch

- **WHEN** the watcher finds a `HotPathReleased` event with no matching `HotPathInitiated`, or with mismatched amount or recipient
- **THEN** the watcher SHALL invoke `pause()` on the affected Bank Contract
- **AND** SHALL log the event to the `treasury` schema for audit

### Requirement: Cold Path rebalances via CCIP burn-and-mint

When a chain's pool drops below its target threshold, the Treasury Service SHALL invoke `rebalance(destChainId, amount)` on a surplus chain's Bank Contract. The Bank Contract SHALL burn the amount locally and SHALL trigger a CCIP burn-and-mint operation that mints the same amount on the destination chain's Bank Contract. Cold path operations SHALL be invisible to end users.

The Treasury Service SHALL NOT submit a rebalance whose destination chain is marked inactive in `RouteReceiver.sol`.

The cold path SHALL only rebalance the **SyncUSD pool** held by Bank Contracts. It SHALL NOT move underlying USDC reserves.

#### Scenario: Treasury rebalances surplus from Tempo to Base

- **WHEN** the Tempo pool depth exceeds its target and the Base pool falls below its threshold
- **THEN** the Treasury Service SHALL invoke `rebalance(BaseChainId, amount)` on the Tempo Bank Contract
- **AND** the Tempo Bank Contract SHALL burn `amount` `SyncUSD` locally
- **AND** the CCIP burn-and-mint operation SHALL mint `amount` `SyncUSD` on the Base Bank Contract
- **AND** the operation SHALL be invisible to end users

#### Scenario: Destination chain is inactive in RouteReceiver

- **WHEN** the Treasury Service plans a rebalance to a chain marked inactive in `RouteReceiver.sol`
- **THEN** the Treasury Service SHALL NOT submit the rebalance

### Requirement: Minimum pool ratio per chain

Each chain SHALL hold a configurable minimum percentage of total `SyncUSD` supply. The Treasury Service SHALL trigger a cold path rebalance proactively when a pool falls below its target threshold — before depletion.

#### Scenario: Pool falls below target threshold

- **WHEN** a chain's pool depth falls below its configured target threshold
- **THEN** the Treasury Service SHALL trigger a cold path rebalance proactively
- **AND** the rebalance SHALL be initiated before the pool is depleted

### Requirement: Audit trail for relays

Every hot path relay and every watcher verification SHALL be recorded in the `treasury` PostgreSQL schema.

#### Scenario: Hot path relay is recorded

- **WHEN** the Treasury Service submits a `releaseHotPath` transaction and the watcher verifies the corresponding `HotPathInitiated` event
- **THEN** both the relay attempt and the watcher verification SHALL be persisted in the `treasury` PostgreSQL schema

### Requirement: Pool depth is on-chain readable

Each Bank Contract SHALL expose a `poolDepth()` view returning the current `SyncUSD` balance held in its local liquidity pool. The Treasury Service SHALL use this to monitor pool depths across chains.

#### Scenario: Treasury reads pool depth across chains

- **WHEN** the Treasury Service queries `poolDepth()` on each active Bank Contract
- **THEN** each call SHALL return the current `SyncUSD` balance held in that chain's local liquidity pool

### Requirement: Audit trail for rebalances

Every cold path rebalance attempt SHALL be recorded in the `treasury.rebalance_ops` table, keyed on the CCIP `messageId`. The record SHALL include source chain, destination chain, amount, status (`initiated`, `completed`, `failed`), and any revert reason. Recording SHALL occur regardless of outcome — including when the on-chain call reverts before a `messageId` is produced (in which case `messageId` is null and the row is keyed on a Treasury-side correlation id).

#### Scenario: Successful rebalance is recorded with messageId

- **WHEN** a rebalance completes on the destination chain
- **THEN** a row SHALL exist in `treasury.rebalance_ops` keyed on the CCIP `messageId`
- **AND** the row SHALL include source chain, destination chain, amount, and status `completed`

#### Scenario: Rebalance reverts before messageId is produced

- **WHEN** the on-chain `rebalance` call reverts before a CCIP `messageId` is emitted
- **THEN** a row SHALL still be written to `treasury.rebalance_ops` with `messageId` null
- **AND** the row SHALL be keyed on a Treasury-side correlation id and SHALL include the revert reason

### Requirement: Cold path respects per-rebalance cap

The Treasury Service SHALL read the destination Bank Contract's `maxRebalanceAmount` and SHALL split any logical rebalance whose total amount exceeds the cap into multiple sequential operations, each within the cap. The on-chain cap is authoritative — Treasury MUST NOT submit operations exceeding it. If Treasury's local cap configuration drifts above the on-chain cap, the on-chain check SHALL revert and Treasury SHALL log and re-plan.

#### Scenario: Logical rebalance exceeds the on-chain cap

- **WHEN** the Treasury Service plans a rebalance whose total amount exceeds the destination Bank Contract's `maxRebalanceAmount`
- **THEN** the Treasury Service SHALL split it into multiple sequential operations
- **AND** each operation SHALL be at most `maxRebalanceAmount`

#### Scenario: Treasury config drifts above on-chain cap

- **WHEN** Treasury submits a rebalance whose `amount` exceeds the on-chain `maxRebalanceAmount`
- **THEN** the on-chain check SHALL revert
- **AND** Treasury SHALL log the failure and re-plan

### Requirement: Incoming cross-chain transfers route to recipient's home chain

When a sender initiates a cross-chain transfer to a known recipient, the BFF SHALL resolve the recipient's `home_chain` via the User Service and SHALL set `destChainId` to that chain in the resulting `transferHotPath` call.

#### Scenario: Alice on Base sends $500 to Bob whose home chain is Tempo

- **WHEN** Alice initiates a transfer to Bob
- **AND** Bob's User Service `home_chain` is Tempo
- **AND** Tempo is active in `RouteReceiver.sol`
- **THEN** the BFF SHALL build a `transferHotPath(Bob, 500, TempoChainId)` call on the Base Bank Contract
- **AND** Bob SHALL receive 500 SyncUSD on Tempo

### Requirement: Routing falls back to sender's chain when home chain is unavailable

The BFF SHALL fall back to same-chain delivery (`destChainId` = sender's chain) in any of the following cases:

- The recipient has no User Service record (e.g., transfer to a raw address that has not onboarded).
- The recipient's `home_chain` is marked inactive in `RouteReceiver.sol`.
- The User Service is unreachable.

Same-chain delivery SHALL never be blocked by these conditions.

#### Scenario: Recipient's home chain is inactive

- **WHEN** Alice on Base sends to Bob whose home chain is Arbitrum
- **AND** Arbitrum is inactive in `RouteReceiver.sol`
- **THEN** the BFF SHALL build the transfer with `destChainId` = Base
- **AND** Bob SHALL receive the SyncUSD on Base
- **AND** Bob's `home_chain` SHALL remain Arbitrum (not auto-reassigned)

### Requirement: USDC reserves are rebalanced across chains independently of SyncUSD pools

The Treasury Service SHALL monitor each Bank Contract's USDC `reserveDepth()` and SHALL invoke `bridgeReserve(destChainId, amount)` when a chain's reserve falls below its configured target threshold. Reserve rebalance SHALL be invisible to end users.

Reserve rebalance and SyncUSD pool rebalance (cold path) SHALL be operated as independent concerns: separate roles, separate audit tables, separate Treasury signers. A failure in one SHALL NOT block the other.

#### Scenario: Base reserve falls below threshold

- **WHEN** the Treasury Service detects Base's `reserveDepth()` is below its target threshold
- **AND** Tempo's reserve has surplus above its target
- **AND** both Tempo and Base are active in `RouteReceiver.sol`
- **THEN** Treasury SHALL invoke `bridgeReserve(BaseChainId, amount)` on the Tempo Bank Contract
- **AND** the operation SHALL be recorded in `treasury.reserve_ops` keyed on the bridge `messageId`

### Requirement: Audit trail for reserve bridges

Every reserve bridge attempt SHALL be recorded in the `treasury.reserve_ops` table with source chain, destination chain, amount, bridge type, bridge `messageId`, status (`initiated`, `completed`, `failed`), and any failure reason. Recording SHALL occur regardless of outcome.

#### Scenario: Failed reserve bridge is still audited

- **WHEN** Treasury attempts a reserve bridge from Tempo to Base
- **AND** the bridge call fails before completion
- **THEN** Treasury SHALL record a `treasury.reserve_ops` row with status `failed`
- **AND** the row SHALL include the source chain, destination chain, amount, bridge type, correlation id or bridge `messageId`, and failure reason

### Requirement: Reserve rebalance respects per-bridge cap

The Treasury Service SHALL split any logical reserve bridge whose total amount exceeds the destination Bank Contract's `maxReserveRebalanceAmount` into multiple sequential operations, each within the cap. The on-chain cap is authoritative.

#### Scenario: Treasury splits a reserve rebalance above the cap

- **WHEN** Base needs 300,000 USDC
- **AND** the source Bank Contract's `maxReserveRebalanceAmount` is 100,000 USDC
- **THEN** Treasury SHALL plan three sequential `bridgeReserve` calls of 100,000 USDC or less
- **AND** Treasury SHALL NOT submit a single call above the on-chain cap

### Requirement: Withdrawal routing is resolved before the transaction is built

Before a withdrawal transaction is constructed, the client SHALL obtain withdrawal routing from the BFF: for each non-decommissioned chain, the user's withdrawable amount, computed by the Treasury Service as the minimum of the user's on-chain `SyncUSD` balance and the chain's reserve depth, with inactive chains reported as unavailable together with a reason. The BFF SHALL NOT compute this itself; it SHALL proxy the Treasury Service.

#### Scenario: User withdraws while their chain is healthy

- **WHEN** Bob requests withdrawal routing and his balance sits on an active chain with sufficient reserve
- **THEN** the routing response SHALL identify that chain and the full withdrawable amount
- **AND** the client SHALL build the `withdraw()` transaction for that chain

#### Scenario: User's chain is inactive

- **WHEN** Bob requests withdrawal routing and his only balance sits on an inactive chain
- **THEN** the routing response SHALL report the amount as temporarily unavailable with the chain-inactive reason
- **AND** the client SHALL NOT build a withdrawal transaction for that chain

#### Scenario: Reserve depth caps the withdrawable amount

- **WHEN** Bob's balance on an active chain exceeds that chain's current reserve depth
- **THEN** the routing response SHALL report the withdrawable amount capped at the reserve depth for that chain

### Requirement: Decommissioned chains are never used as source or destination

The Treasury Service and BFF SHALL treat any chain in `decommissioned` state as terminal. No hot-path transfer, no cold-path rebalance, no reserve bridge SHALL target a decommissioned chain as source or destination. Existing inflight operations targeting a chain that becomes decommissioned mid-flight SHALL be allowed to complete if the destination is the dying chain itself **only** if the operation is part of the drain procedure.

#### Scenario: BFF rejects a transfer to a decommissioned home chain

- **WHEN** a sender initiates a transfer to a recipient whose `home_chain` is decommissioned
- **THEN** the BFF SHALL fall back to same-chain delivery (sender's chain)
- **AND** SHALL NOT update `home_chain` (only the decommission orchestrator may change it)

### Requirement: Treasury executes the chain drain procedure

When a chain is marked for decommissioning, the Treasury Service SHALL execute a drain procedure that, in order:

1. Enumerates SyncUSD holders on the dying chain via its event index.
2. For each holder, bridges the holder's SyncUSD to a governance-chosen target healthy chain via CCIP burn-and-mint, and updates the holder's `home_chain` via the User Service.
3. Drains the SyncUSD pool via `rebalance` to the target chain.
4. Drains the USDC reserve via `bridgeReserve` to the target chain.
5. Records every step in `treasury.decommission_ops` keyed on per-step `messageId` or correlation id.

The drain procedure SHALL be resumable: restart MUST skip already-completed holder bridges based on the audit table.

#### Scenario: Treasury resumes after a partial holder drain

- **WHEN** the decommission orchestrator restarts after one holder bridge is recorded as completed
- **THEN** Treasury SHALL skip the completed holder
- **AND** Treasury SHALL continue draining remaining holders before pool and reserve drain steps

### Requirement: Drain respects activation gate of the target chain

If the drain target chain becomes `inactive` during a drain, Treasury SHALL pause the drain and alert operators. The drain SHALL NOT proceed against an inactive target.

#### Scenario: Target chain becomes inactive during drain

- **WHEN** Treasury detects the selected drain target chain is inactive
- **THEN** Treasury SHALL pause the drain before submitting additional bridge operations
- **AND** Treasury SHALL alert operators for manual governance review

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

