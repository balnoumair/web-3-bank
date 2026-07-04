# Banking Ledger Specification

## Purpose

Represent user balances as on-chain `SyncUSD` tokens, 1:1 backed by externally-held stablecoins (USDC) escrowed in Bank Contracts. The user's balance **is** their token balance — there is no off-chain ledger.
## Requirements
### Requirement: SyncUSD as the unit of account

User balances SHALL be denominated in `SyncUSD`, a stablecoin issued by the bank and backed 1:1 by USDC held in reserve by the Bank Contract on each active chain. The total `SyncUSD` supply across all chains SHALL equal the total underlying USDC reserves.

#### Scenario: Total SyncUSD supply matches total USDC reserves

- **WHEN** the sum of `SyncUSD` total supply across all active chains is computed
- **THEN** it SHALL equal the sum of underlying USDC reserves held by every Bank Contract

### Requirement: One Bank Contract per active chain

Each active chain SHALL host exactly one Bank Contract. The Bank Contract SHALL hold the chain's underlying stablecoin reserve and SHALL manage the chain's local `SyncUSD` liquidity pool.

#### Scenario: A chain in the active set has exactly one Bank Contract

- **WHEN** a chain is in the active set maintained by `RouteReceiver.sol`
- **THEN** exactly one Bank Contract SHALL be deployed on that chain
- **AND** that contract SHALL hold the chain's underlying reserve and manage its local `SyncUSD` pool

### Requirement: Deposit mints SyncUSD against escrowed USDC

When a user deposits an underlying stablecoin into a Bank Contract, the contract SHALL escrow the underlying token in its reserve and SHALL mint an equal amount of `SyncUSD` to the user's address on the same chain.

#### Scenario: Bob deposits $5,000 USDC

- **WHEN** Bob calls `deposit(USDC, 5000)` on the Tempo Bank Contract
- **THEN** 5,000 USDC SHALL be transferred from Bob to the contract's reserve
- **AND** 5,000 `SyncUSD` SHALL be minted to Bob's Tempo address
- **AND** a `Deposited` event SHALL be emitted

### Requirement: Withdraw burns SyncUSD and returns underlying

When a user withdraws, the Bank Contract SHALL burn the user's `SyncUSD` and release an equal amount of the underlying stablecoin from its reserve to the user's wallet. Withdrawal SHALL execute on the chain where the user's `SyncUSD` is held; it SHALL NOT be fulfilled custodially from another chain's pool or reserve. When the user holds `SyncUSD` on multiple chains, withdrawal MAY be executed on any active chain where the user holds balance, up to that chain's balance and reserve depth.

#### Scenario: Bob withdraws $2,000

- **WHEN** Bob calls `withdraw(USDC, 2000)` on a Bank Contract
- **THEN** 2,000 `SyncUSD` SHALL be burned from Bob
- **AND** 2,000 USDC SHALL be released from the reserve to Bob's wallet
- **AND** a `Withdrawn` event SHALL be emitted

#### Scenario: Balance on an inactive chain is reported unavailable, not moved

- **WHEN** Bob's only `SyncUSD` balance is on a chain that is inactive in RouteReceiver
- **THEN** no service SHALL move or release Bob's funds from another chain's pool or reserve
- **AND** the system SHALL report that amount as temporarily unavailable for withdrawal, with the reason
- **AND** the funds SHALL become withdrawable when the chain recovers or after a decommission drain relocates them

### Requirement: Same-chain transfer is a plain token transfer

A transfer between two users on the same chain SHALL execute as a standard ERC-20 `transfer()` of `SyncUSD` between their addresses. The Bank Contract SHALL NOT be involved.

#### Scenario: Alice sends 100 SyncUSD to Bob on the same chain

- **WHEN** Alice calls `transfer(Bob, 100)` on the `SyncUSD` ERC-20 contract on Tempo
- **THEN** 100 `SyncUSD` SHALL move directly from Alice's address to Bob's address
- **AND** the Bank Contract SHALL NOT be invoked

### Requirement: Mint and burn are restricted

`SyncUSD.mint` and `SyncUSD.burn` SHALL be callable only by the Bank Contract on that chain and the CCIP Token Pool. All other callers SHALL be rejected.

#### Scenario: Unauthorized address attempts to mint SyncUSD

- **WHEN** an address that is neither the Bank Contract nor the CCIP Token Pool calls `SyncUSD.mint`
- **THEN** the call SHALL revert

#### Scenario: Bank Contract mints SyncUSD on deposit

- **WHEN** the Bank Contract calls `SyncUSD.mint` as part of a deposit flow
- **THEN** the mint SHALL succeed

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

#### Scenario: Outbound rebalance to a non-allowlisted destination

- **WHEN** an address with `REBALANCER_ROLE` calls `rebalance(destChainId, amount)` with a `destChainId` not in the outbound allowlist
- **THEN** the call SHALL revert without burning any `SyncUSD`

#### Scenario: Inbound CCIP message from a non-allowlisted source

- **WHEN** the Bank Contract receives a CCIP message whose `(srcChainId, srcContract)` pair is not in the inbound allowlist
- **THEN** the receive handler SHALL revert without minting

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

#### Scenario: Reserve bridge rejects unauthorized or unsafe calls

- **WHEN** an address without `RESERVE_REBALANCER_ROLE` calls `bridgeReserve(BaseChainId, 100000)`
- **THEN** the call SHALL revert
- **WHEN** a reserve rebalancer calls `bridgeReserve(BaseChainId, amount)` with `amount` above `maxReserveRebalanceAmount`
- **THEN** the call SHALL revert
- **WHEN** the destination chain is not active
- **THEN** the call SHALL revert
- **WHEN** the destination Bank Contract receives the same inbound `messageId` twice
- **THEN** the second delivery SHALL revert

### Requirement: Reserve bridging is pluggable per chain

Each Bank Contract SHALL hold a reference to an `IReserveBridge` adapter, settable only by governance. The adapter encapsulates the chain-specific bridge mechanism (e.g., Circle CCTP). The Bank Contract SHALL revert any `bridgeReserve` call if no adapter is set.

#### Scenario: Governance registers the reserve bridge adapter

- **WHEN** governance calls `setReserveBridge(adapterAddress)`
- **THEN** the Bank Contract SHALL store `adapterAddress` as the registered reserve bridge
- **AND** subsequent `bridgeReserve` calls SHALL use that adapter
- **WHEN** no adapter is registered
- **THEN** `bridgeReserve` SHALL revert before moving funds

### Requirement: Reserve depth is on-chain readable

Each Bank Contract SHALL expose a `reserveDepth()` view returning the current USDC balance held in its reserve. The Treasury Service SHALL use this to monitor reserve depths across chains.

#### Scenario: Treasury reads current reserve depth

- **WHEN** the Bank Contract holds 250,000 USDC in reserve
- **THEN** `reserveDepth()` SHALL return 250,000
- **AND** the Treasury Service SHALL use that value when planning reserve rebalance operations

### Requirement: Bank Contract supports a freeze-for-decommission state

Bank Contracts SHALL expose an admin-only one-shot `freezeForDecommission()` function. Once called:

- `deposit`, `transferHotPath` (as source), and `releaseHotPath` (as destination) SHALL revert.
- `withdraw` SHALL remain available throughout the governance-defined grace period.
- `rebalance` and `bridgeReserve` SHALL remain available so the Treasury Service can drain pool and reserve.

Freeze SHALL NOT be reversible.

#### Scenario: Frozen Bank Contract rejects a deposit

- **WHEN** the Bank Contract is in the frozen state
- **AND** a user calls `deposit(USDC, 1000)`
- **THEN** the call SHALL revert
- **AND** the same contract SHALL still accept `withdraw` and Treasury-initiated drain operations

### Requirement: Bank Contract supports permanent pause

Bank Contracts SHALL expose an admin-only `pausePermanently()` function, called by governance after drain completes. The contract SHALL use the existing `Pausable` mechanism but SHALL NOT support unpause from the permanent state. After permanent pause, all operations SHALL revert.

#### Scenario: Permanently paused Bank Contract cannot be unpaused

- **WHEN** governance calls `pausePermanently()` after drain completion
- **AND** a pauser attempts to call `unpause()`
- **THEN** the call SHALL revert
- **AND** the Bank Contract SHALL remain paused

