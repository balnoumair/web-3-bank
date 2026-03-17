# SyncUSD Token Contract — Design Spec

**Date:** 2026-03-17
**Task:** 01-syncusd-token-contract
**Status:** Approved

---

## Overview

Implement the SyncUSD stablecoin ERC-20 token with CCIP burn-and-mint support, role-based access control, UUPS upgradeability, and pausability. Added to the existing Foundry project at `packages/cre-contracts/foundry/`.

---

## Location

All files under `packages/cre-contracts/foundry/`:

```
src/
  SyncUSD.sol
  interfaces/
    IBurnMintERC20.sol     # Chainlink CCIP burn-and-mint interface
lib/
  openzeppelin-contracts-upgradeable/
  chainlink-ccip/          # new dependency (for IBurnMintERC20)
test/
  SyncUSD.t.sol
```

---

## Contract Architecture

**Inheritance chain:**

```
SyncUSD
  ├── ERC20Upgradeable         (OpenZeppelin)
  ├── AccessControlUpgradeable (OpenZeppelin)
  ├── PausableUpgradeable      (OpenZeppelin)
  ├── UUPSUpgradeable          (OpenZeppelin)
  └── IBurnMintERC20           (Chainlink CCIP interface)
```

SyncUSD directly implements `IBurnMintERC20` — the canonical Chainlink burn-and-mint pattern. The CCIP Token Pool calls `mint`/`burn` on the token contract itself, both gated by `MINTER_ROLE`. No adapter contract needed.

---

## Token Metadata

| Property | Value |
|----------|-------|
| Name | `SyncUSD` |
| Symbol | `sUSD` |
| Decimals | `6` (matches USDC) |
| Standard | ERC-20 (TIP-20 on Tempo, ERC-20 elsewhere) |

---

## Roles & Access Control

| Role | Bytes32 Constant | Assigned To | Permissions |
|------|-----------------|-------------|-------------|
| `DEFAULT_ADMIN_ROLE` | `0x00` (OZ default) | Timelock | Grant/revoke all roles |
| `ADMIN_ROLE` | `keccak256("ADMIN_ROLE")` | Timelock | `_authorizeUpgrade` |
| `MINTER_ROLE` | `keccak256("MINTER_ROLE")` | Bank Contract, CCIP Token Pool | `mint()`, `burn()`, `burnFrom()` |
| `PAUSER_ROLE` | `keccak256("PAUSER_ROLE")` | Emergency multisig | `pause()`, `unpause()` |

`DEFAULT_ADMIN_ROLE` guards role management (OZ standard). `ADMIN_ROLE` is a separate explicit role for UUPS upgrades, both protected by Timelock in production.

---

## Key Functions

```solidity
// Initializer — replaces constructor for UUPS proxy pattern
function initialize(address admin, address pauser) external initializer

// CCIP IBurnMintERC20 interface
function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE)
function burn(uint256 amount) external onlyRole(MINTER_ROLE)
function burnFrom(address from, uint256 amount) external onlyRole(MINTER_ROLE)

// Transfer overrides — revert when paused
function transfer(address to, uint256 amount) public override whenNotPaused returns (bool)
function transferFrom(address from, address to, uint256 amount) public override whenNotPaused returns (bool)

// UUPS upgrade guard
function _authorizeUpgrade(address newImpl) internal override onlyRole(ADMIN_ROLE)

// ERC-165 interface detection (required by CCIP)
function supportsInterface(bytes4 interfaceId) public view override returns (bool)
```

---

## Pausability

When paused (triggered by `PAUSER_ROLE`):
- `mint` reverts
- `burn` / `burnFrom` revert
- `transfer` / `transferFrom` revert

Unpausing restores all functionality. The pause state does not affect role management or view functions.

---

## UUPS Upgradeability

- Proxy pattern: OpenZeppelin `UUPSUpgradeable`
- `initialize()` replaces constructor; called once on proxy deployment
- `_authorizeUpgrade` gated to `ADMIN_ROLE` (Timelock in production)
- Storage layout must be preserved across upgrades (OZ upgrade safety)

---

## CCIP Integration

Implements Chainlink's `IBurnMintERC20` interface:

```solidity
interface IBurnMintERC20 {
    function mint(address account, uint256 amount) external;
    function burn(uint256 amount) external;
    function burnFrom(address account, uint256 amount) external;
}
```

`supportsInterface` returns `true` for `IBurnMintERC20`'s interface ID, enabling CCIP Token Pool to detect and call the token directly. No separate adapter contract is needed for this implementation.

---

## Dependencies to Add

```toml
# foundry.toml — no changes needed (already Solidity 0.8.24)
```

```bash
# Add via forge install:
forge install OpenZeppelin/openzeppelin-contracts-upgradeable --no-commit
forge install smartcontractkit/ccip --no-commit  # for IBurnMintERC20
```

---

## Tests (SyncUSD.t.sol)

| Category | Test Cases |
|----------|-----------|
| Deployment | `initialize` sets admin/pauser roles; decimals = 6; not paused; cannot re-initialize |
| Mint | MINTER_ROLE mints successfully; emits Transfer; unauthorized reverts with AccessControl error |
| Burn | MINTER_ROLE burns own balance; `burnFrom` with sufficient allowance; reverts if over balance |
| Pause | PAUSER_ROLE pauses; `transfer` reverts; `mint` reverts; `burn` reverts; `unpause` restores all |
| Upgrade | ADMIN_ROLE can upgrade proxy; unauthorized address reverts; storage preserved after upgrade |
| CCIP | `supportsInterface(IBurnMintERC20_ID)` returns true; `supportsInterface(IERC20_ID)` returns true |
| Access Control | Grant MINTER_ROLE via DEFAULT_ADMIN_ROLE; revoke removes permission; non-admin cannot grant |

---

## Acceptance Criteria

- `forge build` compiles without errors or warnings
- `forge test` passes all tests
- Contract is deployable to Anvil local devnet via `forge script`
