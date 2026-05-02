# Banking Ledger — Delta

## ADDED Requirements

### Requirement: Bank Contract bridges USDC reserves across chains

Bank Contracts SHALL expose a permissioned `bridgeReserve(uint64 destChainId, uint256 amount)` function that moves `amount` USDC from this chain's reserve to the destination chain's reserve via a registered bridge adapter (`IReserveBridge`). This is the only mechanism by which USDC reserves move between chains under normal operation.

This function SHALL NOT mint, burn, or move SyncUSD. SyncUSD pool liquidity is managed by the cold path (`rebalance`), which is a separate concern.

#### Scenario: Treasury bridges 100,000 USDC from Tempo to Base reserve

- **WHEN** the Treasury Service calls `bridgeReserve(BaseChainId, 100000)` on the Tempo Bank Contract
- **THEN** 100,000 USDC SHALL be transferred from Tempo's reserve to the registered bridge adapter
- **AND** a `ReserveBridgeInitiated` event SHALL be emitted with the bridge `messageId`
- **AND** upon bridge delivery, the Base Bank Contract's reserve SHALL increase by 100,000 USDC
- **AND** the Base Bank Contract SHALL emit a `ReserveBridgeCompleted` event with the same `messageId`

### Requirement: Reserve bridge is restricted, capped, idempotent, and activation-gated

`bridgeReserve` SHALL be callable only by an address holding `RESERVE_REBALANCER_ROLE`. `RESERVE_REBALANCER_ROLE` SHALL be distinct from both `RELAYER_ROLE` and `REBALANCER_ROLE`. Each call SHALL be rejected if `amount` exceeds the contract's configured `maxReserveRebalanceAmount`. The destination chain SHALL be active in `RouteReceiver.sol` at call time. The destination Bank Contract SHALL reject any inbound bridge message whose `messageId` it has already processed.

### Requirement: Reserve bridging is pluggable per chain

Each Bank Contract SHALL hold a reference to an `IReserveBridge` adapter, settable only by governance. The adapter encapsulates the chain-specific bridge mechanism (e.g., Circle CCTP). The Bank Contract SHALL revert any `bridgeReserve` call if no adapter is set.

### Requirement: Reserve depth is on-chain readable

Each Bank Contract SHALL expose a `reserveDepth()` view returning the current USDC balance held in its reserve. The Treasury Service SHALL use this to monitor reserve depths across chains.
