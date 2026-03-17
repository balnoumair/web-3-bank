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
    IBurnMintERC20.sol     # verbatim copy of Chainlink's canonical interface
lib/
  openzeppelin-contracts-upgradeable/
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

**Note on `ERC20BurnableUpgradeable`:** Do NOT inherit `ERC20BurnableUpgradeable`. That contract exposes a public ungated `burn(uint256)` which would bypass `MINTER_ROLE`. Instead, call `_burn` (the internal OZ function) directly inside the gated `burn` and `burnFrom` overrides.

**Note on constructor:** The implementation contract must include a constructor that calls `_disableInitializers()` to prevent the logic contract itself from being initialized and exploited:
```solidity
/// @custom:oz-upgrades-unsafe-allow constructor
constructor() {
    _disableInitializers();
}
```

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

**MINTER_ROLE grant strategy:** The `initialize` function accepts no minter address — `MINTER_ROLE` is granted post-deploy by the admin (via `grantRole`) once the Bank Contract and CCIP Token Pool addresses are known. The test suite must include a test verifying that minting reverts before the role is granted.

---

## Key Functions

```solidity
// Constructor — disables initializers on the logic contract
/// @custom:oz-upgrades-unsafe-allow constructor
constructor() { _disableInitializers(); }

// Initializer — replaces constructor for UUPS proxy pattern
// Grants DEFAULT_ADMIN_ROLE and ADMIN_ROLE to `admin`, PAUSER_ROLE to `pauser`
// MINTER_ROLE is NOT granted here — granted post-deploy via grantRole
function initialize(address admin, address pauser) external initializer

// CCIP IBurnMintERC20 interface — all blocked when paused
function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) whenNotPaused
function burn(uint256 amount) external onlyRole(MINTER_ROLE) whenNotPaused
function burnFrom(address from, uint256 amount) external onlyRole(MINTER_ROLE) whenNotPaused

// Transfer overrides — revert when paused
function transfer(address to, uint256 amount) public override whenNotPaused returns (bool)
function transferFrom(address from, address to, uint256 amount) public override whenNotPaused returns (bool)

// UUPS upgrade guard
function _authorizeUpgrade(address newImpl) internal override onlyRole(ADMIN_ROLE)

// Decimals override — ERC20Upgradeable defaults to 18; must be overridden to return 6
function decimals() public pure override returns (uint8) { return 6; }

// ERC-165 interface detection (required by CCIP)
// Must call super.supportsInterface(interfaceId) and OR in the IBurnMintERC20 interface ID
// Pattern: return interfaceId == type(IBurnMintERC20).interfaceId || super.supportsInterface(interfaceId)
// IMPORTANT: List ALL base classes that define supportsInterface in the override specifier.
// For OZ upgradeable v5.x, only AccessControlUpgradeable defines it, so override(AccessControlUpgradeable)
// is correct. Verify against the installed OZ version — if additional bases define it, list them all.
function supportsInterface(bytes4 interfaceId) public view override(AccessControlUpgradeable) returns (bool)
```

---

## IBurnMintERC20 Interface

The local `src/interfaces/IBurnMintERC20.sol` must be a verbatim copy of Chainlink's canonical definition. The real interface inherits from `IERC20` and `IERC165`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC165} from "@openzeppelin/contracts/utils/introspection/IERC165.sol";

interface IBurnMintERC20 is IERC20, IERC165 {
    function mint(address account, uint256 amount) external;
    function burn(uint256 amount) external;
    function burnFrom(address account, uint256 amount) external;
}
```

This interface surface must be fully satisfied by SyncUSD. `supportsInterface` must return `true` for `type(IBurnMintERC20).interfaceId`.

---

## Pausability

When paused (triggered by `PAUSER_ROLE`):
- `mint` reverts (`whenNotPaused`)
- `burn` / `burnFrom` revert (`whenNotPaused`)
- `transfer` / `transferFrom` revert (`whenNotPaused`)

Unpausing restores all functionality. Pause does not affect role management, view functions, or `_authorizeUpgrade`.

---

## UUPS Upgradeability

- Proxy pattern: OpenZeppelin `UUPSUpgradeable`
- Logic contract constructor calls `_disableInitializers()` (prevents direct initialization attack)
- `initialize()` is called once on the proxy at deployment
- `_authorizeUpgrade` gated to `ADMIN_ROLE` (Timelock in production)
- Storage layout must be preserved across upgrades (OZ upgrade safety)

---

## Dependencies to Add

```bash
# Add via forge install (run from packages/cre-contracts/foundry/):
forge install OpenZeppelin/openzeppelin-contracts-upgradeable --no-commit
```

The lib folder will contain:
- `lib/openzeppelin-contracts-upgradeable/`

**Note on Chainlink CCIP:** Do NOT install `smartcontractkit/chainlink` — it is a multi-gigabyte monorepo. The `IBurnMintERC20` interface is vendored locally at `src/interfaces/IBurnMintERC20.sol` as a verbatim copy of Chainlink's canonical definition. No `forge install` for Chainlink is needed.

Update `foundry.toml` remappings accordingly:
```toml
remappings = [
  "@openzeppelin/contracts-upgradeable/=lib/openzeppelin-contracts-upgradeable/contracts/",
  "@openzeppelin/contracts/=lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/",
]
```

---

## Tests (SyncUSD.t.sol)

| Category | Test Cases |
|----------|-----------|
| Deployment | `initialize` sets admin/pauser roles; decimals = 6; not paused; cannot re-initialize; MINTER_ROLE not granted at init |
| Pre-mint gate | Minting reverts before MINTER_ROLE is granted |
| Mint | MINTER_ROLE mints successfully; emits Transfer; unauthorized reverts with AccessControl error |
| Burn | MINTER_ROLE burns own balance via `burn`; `burnFrom` burns with sufficient allowance; `burnFrom` reverts if allowance insufficient; reverts if over balance |
| Pause | PAUSER_ROLE pauses; `transfer` reverts; `mint` reverts; `burn` reverts; `burnFrom` reverts; `unpause` restores all |
| Upgrade | ADMIN_ROLE can upgrade proxy; unauthorized address reverts; storage (balances) preserved after upgrade |
| CCIP | `supportsInterface(IBurnMintERC20_ID)` returns true; `supportsInterface(IERC20_ID)` returns true; `supportsInterface(IAccessControl_ID)` returns true |
| Access Control | Grant MINTER_ROLE via DEFAULT_ADMIN_ROLE; revoke removes permission; non-admin cannot grant |

---

## Acceptance Criteria

- `forge build` compiles without errors or warnings
- `forge test` passes all tests
- Contract is deployable to Anvil local devnet via `forge script`
