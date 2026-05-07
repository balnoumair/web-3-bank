# Banking Ledger — Delta

## ADDED Requirements

### Requirement: Rebalance moves SyncUSD pool liquidity across chains

Bank Contracts SHALL expose a permissioned `rebalance(uint64 destChainId, uint256 amount)` function. When called, the contract SHALL burn `amount` SyncUSD from its local liquidity pool and SHALL send a CCIP message instructing the destination chain's Bank Contract to mint the same amount into its local pool. This function SHALL be the only mechanism by which SyncUSD pool liquidity moves between chains under normal operation.

The function SHALL NOT move USDC reserves. Underlying reserves remain on the chain where they were deposited.

#### Scenario: Treasury rebalances 50,000 SyncUSD from Tempo to Base

- **WHEN** the Treasury Service calls `rebalance(BaseChainId, 50000)` on the Tempo Bank Contract
- **THEN** 50,000 SyncUSD SHALL be burned from the Tempo pool
- **AND** a CCIP message SHALL be dispatched to the Base Bank Contract
- **AND** a `RebalanceInitiated` event SHALL be emitted carrying the CCIP `messageId`
- **AND** upon CCIP delivery, the Base Bank Contract SHALL mint 50,000 SyncUSD into its local pool
- **AND** the Base Bank Contract SHALL emit a `RebalanceCompleted` event carrying the same `messageId`

### Requirement: Rebalance is restricted, capped, and idempotent

`rebalance` SHALL be callable only by an address holding `REBALANCER_ROLE`. `REBALANCER_ROLE` SHALL be distinct from `RELAYER_ROLE`. Each call SHALL be rejected if `amount` exceeds the contract's configured `maxRebalanceAmount`. The destination chain's Bank Contract SHALL reject any inbound CCIP message whose `messageId` it has already processed.

#### Scenario: Unauthorized rebalance attempt

- **WHEN** an address without `REBALANCER_ROLE` calls `rebalance`
- **THEN** the call SHALL revert

#### Scenario: Rebalance amount exceeds cap

- **WHEN** Treasury calls `rebalance(destChainId, amount)` with `amount > maxRebalanceAmount`
- **THEN** the call SHALL revert without burning any SyncUSD

#### Scenario: Replayed CCIP delivery

- **WHEN** the destination Bank Contract receives a CCIP message whose `messageId` it has already processed
- **THEN** the contract SHALL revert without minting

### Requirement: Rebalance respects CCIP allowlists

The Bank Contract SHALL maintain an allowlist of permitted outbound destination chain IDs and an allowlist of permitted inbound source contracts (keyed by source chain ID and source contract address). Outbound `rebalance` calls to non-allowlisted destinations SHALL revert. Inbound CCIP messages from non-allowlisted sources SHALL revert.
