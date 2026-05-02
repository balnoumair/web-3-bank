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

### Requirement: Reserve rebalance respects per-bridge cap

The Treasury Service SHALL split any logical reserve bridge whose total amount exceeds the destination Bank Contract's `maxReserveRebalanceAmount` into multiple sequential operations, each within the cap. The on-chain cap is authoritative.
