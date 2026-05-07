# Cross-Chain Routing — Delta

## ADDED Requirements

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
