# Cross-Chain Routing — Delta

## ADDED Requirements

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
