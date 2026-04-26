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

When a chain's pool drops below its target threshold, the Treasury Service SHALL initiate a CCIP burn-and-mint operation to move `SyncUSD` liquidity from a surplus chain to the deficit chain. Cold path operations SHALL be batched for gas efficiency and SHALL be invisible to end users.

### Requirement: Minimum pool ratio per chain

Each chain SHALL hold a configurable minimum percentage of total `SyncUSD` supply. The Treasury Service SHALL trigger a cold path rebalance proactively when a pool falls below its target threshold — before depletion.

### Requirement: Audit trail for relays

Every hot path relay and every watcher verification SHALL be recorded in the `treasury` PostgreSQL schema.

### Requirement: Pool depth is on-chain readable

Each Bank Contract SHALL expose a `poolDepth()` view returning the current `SyncUSD` balance held in its local liquidity pool. The Treasury Service SHALL use this to monitor pool depths across chains.
