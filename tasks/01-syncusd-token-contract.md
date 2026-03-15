# Task 01: SyncUSD Token Contract

**Service:** `contracts`
**Depends on:** None
**Can parallelize with:** Task 02, Task 05, Task 06, Task 07

## Goal

Implement the SyncUSD stablecoin token contract with CCIP burn-and-mint support, access control, upgradeability, and pausability.

## Scope

### Foundry Project Setup
- Initialize Foundry project at `contracts/`
- Add OpenZeppelin contracts as dependency
- Add Chainlink CCIP contracts as dependency
- Configure `foundry.toml` for Solidity 0.8.24+

### SyncUSD Contract
- ERC-20 token with 6 decimals (matching USDC)
- OpenZeppelin `AccessControl`:
  - `MINTER_ROLE` — restricted to Bank Contract and CCIP Token Pool
  - `ADMIN_ROLE` — protected by Timelock
  - `PAUSER_ROLE` — for emergency pause
- OpenZeppelin `UUPSUpgradeable`
- OpenZeppelin `Pausable` — disables `mint`, `burn`, `transfer`, `transferFrom` when paused
- `mint(address to, uint256 amount)` — restricted to `MINTER_ROLE`
- `burn(uint256 amount)` — restricted to `MINTER_ROLE`
- Standard `transfer`, `transferFrom` — pausable

### CCIP Extensions
- Implement Chainlink `BurnMintERC20` interface for cross-chain burn-and-mint
- Token pool adapter for CCIP integration

### Tests
- Unit tests for all functions
- Access control tests (unauthorized callers revert)
- Pause/unpause behavior
- UUPS upgrade test
- CCIP burn-and-mint flow test

## Acceptance Criteria
- `forge build` compiles without errors
- `forge test` passes all tests
- Contract is deployable to Anvil local devnet
