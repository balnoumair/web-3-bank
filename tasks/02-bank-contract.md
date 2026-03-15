# Task 02: Bank Contract (Liquidity Pool)

**Service:** `contracts`
**Depends on:** Task 01 (SyncUSD must exist for integration)
**Can parallelize with:** Task 05, Task 06, Task 07

## Goal

Implement the Bank Contract that manages deposits, withdrawals, and hot path transfers. One instance is deployed per chain.

## Scope

### Bank Contract
- OpenZeppelin `AccessControl`:
  - `RELAYER_ROLE` — Treasury Service relayer (can call `releaseHotPath`)
  - `ADMIN_ROLE` — protected by Timelock
  - `PAUSER_ROLE` — for emergency pause
- OpenZeppelin `UUPSUpgradeable`
- OpenZeppelin `Pausable`

### Core Methods
- `deposit(address underlyingToken, uint256 amount)` — Escrows USDC, mints equivalent SyncUSD to caller. Pausable.
- `withdraw(address underlyingToken, uint256 amount)` — Burns caller's SyncUSD, releases equivalent USDC. Pausable.
- `transferHotPath(address to, uint256 amount, uint256 destinationChainId)` — Locks sender's SyncUSD in pool. Emits `HotPathInitiated` event with all transfer details. Pausable.
- `releaseHotPath(address to, uint256 amount, bytes32 sourceEventHash)` — Releases SyncUSD from pool to recipient. Restricted to `RELAYER_ROLE`. Emits `HotPathReleased` event. Pausable.

### Events
- `HotPathInitiated(address indexed sender, address indexed to, uint256 amount, uint256 destinationChainId, bytes32 eventHash)`
- `HotPathReleased(address indexed to, uint256 amount, bytes32 indexed sourceEventHash)`
- `Deposited(address indexed user, address underlyingToken, uint256 amount)`
- `Withdrawn(address indexed user, address underlyingToken, uint256 amount)`

### Fee Interface (Reserved, set to 0)
- Fee parameter in deposit/withdraw (currently 0)
- Fee field in hot path event payload
- Fee collection address configurable by `ADMIN_ROLE`

### Tests
- Deposit/withdraw round-trip
- Hot path initiate + release flow
- Access control (only `RELAYER_ROLE` can `releaseHotPath`)
- Pause blocks all state-mutating functions
- Insufficient pool liquidity reverts `releaseHotPath`
- UUPS upgrade test

## Acceptance Criteria
- `forge build` compiles without errors
- `forge test` passes all tests
- Full deposit → hot path → release flow works on Anvil
