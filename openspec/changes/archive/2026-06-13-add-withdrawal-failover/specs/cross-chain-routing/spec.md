# Cross-Chain Routing — Delta

## ADDED Requirements

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
