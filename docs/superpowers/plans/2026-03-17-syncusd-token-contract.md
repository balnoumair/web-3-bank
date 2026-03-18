# SyncUSD Token Contract Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the SyncUSD stablecoin ERC-20 token with CCIP burn-and-mint support, role-based access control, UUPS upgradeability, and pausability inside the existing Foundry project.

**Architecture:** Single contract inheriting OZ upgradeable base contracts and implementing Chainlink's `IBurnMintERC20` interface directly. The `IBurnMintERC20` interface is vendored locally (no Chainlink monorepo install). `MINTER_ROLE` gates all mint/burn operations; `PAUSER_ROLE` blocks token transfers; `ADMIN_ROLE` guards upgrades.

**Tech Stack:** Solidity 0.8.24, Foundry (forge/anvil), OpenZeppelin Contracts Upgradeable v5.x, forge-std

**Spec:** `docs/superpowers/specs/2026-03-17-syncusd-token-contract-design.md`

---

## File Map

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `packages/cre-contracts/foundry/foundry.toml` | Add OZ remappings |
| Create | `packages/cre-contracts/foundry/src/interfaces/IBurnMintERC20.sol` | Vendored Chainlink CCIP interface |
| Create | `packages/cre-contracts/foundry/src/SyncUSD.sol` | Token contract |
| Create | `packages/cre-contracts/foundry/test/SyncUSD.t.sol` | All unit tests |

All commands run from: `packages/cre-contracts/foundry/`

---

## Chunk 1: Setup — Dependency, Remappings, Interface

### Task 1: Install OpenZeppelin upgradeable contracts

- [ ] **Step 1.1: Navigate to Foundry project root**

```bash
cd packages/cre-contracts/foundry
```

- [ ] **Step 1.2: Install OpenZeppelin contracts-upgradeable**

```bash
forge install OpenZeppelin/openzeppelin-contracts-upgradeable --no-commit
```

Expected: Directory `lib/openzeppelin-contracts-upgradeable/` appears. No errors.

- [ ] **Step 1.3: Verify the install**

```bash
ls lib/openzeppelin-contracts-upgradeable/contracts/token/ERC20/ERC20Upgradeable.sol
```

Expected: File exists (no "No such file" error).

- [ ] **Step 1.4: Update `foundry.toml` to add remappings**

Add `remappings` to the `[profile.default]` section. The full updated file should look like:

```toml
[profile.default]
src = "src"
out = "out"
test = "test"
libs = ["lib"]
solc_version = "0.8.24"
remappings = [
  "@openzeppelin/contracts-upgradeable/=lib/openzeppelin-contracts-upgradeable/contracts/",
  "@openzeppelin/contracts/=lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/",
]

# See https://book.getfoundry.sh/reference/config/ for more options.
```

- [ ] **Step 1.5: Verify remappings resolve**

```bash
forge remappings
```

Expected output includes:
```
@openzeppelin/contracts-upgradeable/=lib/openzeppelin-contracts-upgradeable/contracts/
@openzeppelin/contracts/=lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/
```

- [ ] **Step 1.6: Confirm existing tests still pass after remapping change**

```bash
forge test --match-path "test/RouteReceiver.t.sol" -v
```

Expected: All passing (no regressions).

- [ ] **Step 1.7: Commit**

```bash
# Stage files relative to foundry/ CWD
git add foundry.toml foundry.lock lib/openzeppelin-contracts-upgradeable
# .gitmodules lives at the repo root, not in foundry/ — stage it from root
git -C ../../.. add .gitmodules
git commit -m "chore(contracts): add OZ upgradeable dependency and remappings"
```

---

### Task 2: Create the IBurnMintERC20 vendor interface

- [ ] **Step 2.1: Create the interfaces directory**

```bash
mkdir -p src/interfaces
```

- [ ] **Step 2.2: Create `src/interfaces/IBurnMintERC20.sol`**

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC165} from "@openzeppelin/contracts/utils/introspection/IERC165.sol";

/// @notice Chainlink CCIP burn-and-mint interface.
/// @dev Vendored verbatim from Chainlink's canonical definition.
///      The CCIP Token Pool expects this interface to be satisfied by the token.
interface IBurnMintERC20 is IERC20, IERC165 {
    function mint(address account, uint256 amount) external;
    function burn(uint256 amount) external;
    function burnFrom(address account, uint256 amount) external;
}
```

- [ ] **Step 2.3: Verify it compiles**

```bash
forge build
```

Expected: `Compiler run successful`

- [ ] **Step 2.4: Commit**

```bash
git add src/interfaces/IBurnMintERC20.sol
git commit -m "feat(contracts): vendor IBurnMintERC20 interface"
```

---

## Chunk 2: Contract Skeleton + Deployment Tests

### Task 3: Write deployment tests (TDD — tests first)

- [ ] **Step 3.1: Create `test/SyncUSD.t.sol` with deployment tests**

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {SyncUSD} from "../src/SyncUSD.sol";

contract SyncUSDTest is Test {
    SyncUSD public implementation;
    SyncUSD public token; // proxy cast to SyncUSD

    address public admin = address(0xA1);
    address public pauser = address(0xA2);
    address public minter = address(0xA3);
    address public user = address(0xA4);
    address public unauthorized = address(0xA5);

    function setUp() public {
        implementation = new SyncUSD();
        bytes memory initData = abi.encodeCall(SyncUSD.initialize, (admin, pauser));
        ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);
        token = SyncUSD(address(proxy));
    }

    // ── Deployment ──────────────────────────────────────────────────────

    function test_decimalsIsSix() public view {
        assertEq(token.decimals(), 6);
    }

    function test_nameAndSymbol() public view {
        assertEq(token.name(), "SyncUSD");
        assertEq(token.symbol(), "sUSD");
    }

    function test_adminHasDefaultAdminRole() public view {
        assertTrue(token.hasRole(token.DEFAULT_ADMIN_ROLE(), admin));
    }

    function test_adminHasAdminRole() public view {
        bytes32 ADMIN_ROLE = keccak256("ADMIN_ROLE");
        assertTrue(token.hasRole(ADMIN_ROLE, admin));
    }

    function test_pauserHasPauserRole() public view {
        bytes32 PAUSER_ROLE = keccak256("PAUSER_ROLE");
        assertTrue(token.hasRole(PAUSER_ROLE, pauser));
    }

    function test_minterRoleNotGrantedAtInit() public view {
        bytes32 MINTER_ROLE = keccak256("MINTER_ROLE");
        assertFalse(token.hasRole(MINTER_ROLE, admin));
        assertFalse(token.hasRole(MINTER_ROLE, pauser));
    }

    function test_notPausedAfterInit() public view {
        assertFalse(token.paused());
    }

    function test_cannotReinitialize() public {
        vm.expectRevert();
        token.initialize(admin, pauser);
    }

    function test_implementationCannotBeInitialized() public {
        vm.expectRevert();
        implementation.initialize(admin, pauser);
    }
}
```

- [ ] **Step 3.2: Run to confirm tests fail (contract doesn't exist yet)**

```bash
forge test --match-path "test/SyncUSD.t.sol" -v 2>&1 | head -20
```

Expected: Compilation error — `SyncUSD` not found.

---

### Task 4: Implement the SyncUSD contract skeleton

- [ ] **Step 4.1: Create `src/SyncUSD.sol`**

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ERC20Upgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol";
import {AccessControlUpgradeable} from "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IBurnMintERC20} from "./interfaces/IBurnMintERC20.sol";

/// @title SyncUSD
/// @notice Multi-chain stablecoin token with CCIP burn-and-mint support.
/// @dev UUPS upgradeable. Roles are gated via AccessControl.
///      - MINTER_ROLE: Bank Contract and CCIP Token Pool — may mint/burn
///      - ADMIN_ROLE:  Timelock — may authorize upgrades
///      - PAUSER_ROLE: Emergency multisig — may pause/unpause
///      - DEFAULT_ADMIN_ROLE: Timelock — may grant/revoke roles
contract SyncUSD is
    Initializable,
    ERC20Upgradeable,
    AccessControlUpgradeable,
    PausableUpgradeable,
    UUPSUpgradeable,
    IBurnMintERC20
{
    // ── Roles ──────────────────────────────────────────────────────────

    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    // ── Constructor ────────────────────────────────────────────────────

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    // ── Initializer ────────────────────────────────────────────────────

    /// @notice Initializes the proxy. Called once by the deployer.
    /// @param admin  Receives DEFAULT_ADMIN_ROLE and ADMIN_ROLE (Timelock in production).
    /// @param pauser Receives PAUSER_ROLE (emergency multisig in production).
    /// @dev MINTER_ROLE is not granted here — granted post-deploy via grantRole.
    function initialize(address admin, address pauser) external initializer {
        __ERC20_init("SyncUSD", "sUSD");
        __AccessControl_init();
        __Pausable_init();
        __UUPSUpgradeable_init();

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, pauser);
    }

    // ── ERC-20 overrides ───────────────────────────────────────────────

    /// @inheritdoc ERC20Upgradeable
    function decimals() public pure override returns (uint8) {
        return 6;
    }

    /// @notice Transfers are blocked when the contract is paused.
    function transfer(address to, uint256 amount)
        public
        override(ERC20Upgradeable, IBurnMintERC20)
        whenNotPaused
        returns (bool)
    {
        return super.transfer(to, amount);
    }

    /// @notice TransferFrom is blocked when the contract is paused.
    function transferFrom(address from, address to, uint256 amount)
        public
        override(ERC20Upgradeable, IBurnMintERC20)
        whenNotPaused
        returns (bool)
    {
        return super.transferFrom(from, to, amount);
    }

    // ── IBurnMintERC20 ─────────────────────────────────────────────────

    /// @notice Mints `amount` tokens to `to`. Restricted to MINTER_ROLE.
    function mint(address to, uint256 amount)
        external
        override
        onlyRole(MINTER_ROLE)
        whenNotPaused
    {
        _mint(to, amount);
    }

    /// @notice Burns `amount` from msg.sender's balance. Restricted to MINTER_ROLE.
    /// @dev Does NOT inherit ERC20BurnableUpgradeable — that would expose an ungated public burn.
    function burn(uint256 amount)
        external
        override
        onlyRole(MINTER_ROLE)
        whenNotPaused
    {
        _burn(msg.sender, amount);
    }

    /// @notice Burns `amount` from `from`'s balance using caller's allowance.
    /// @dev Caller must have MINTER_ROLE and sufficient allowance from `from`.
    function burnFrom(address from, uint256 amount)
        external
        override
        onlyRole(MINTER_ROLE)
        whenNotPaused
    {
        _spendAllowance(from, msg.sender, amount);
        _burn(from, amount);
    }

    // ── Pause ──────────────────────────────────────────────────────────

    /// @notice Pauses all token transfers, mints, and burns.
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    /// @notice Unpauses the contract.
    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    // ── UUPS ───────────────────────────────────────────────────────────

    /// @dev Only ADMIN_ROLE may authorize upgrades.
    function _authorizeUpgrade(address newImplementation)
        internal
        override
        onlyRole(ADMIN_ROLE)
    {}

    // ── ERC-165 ────────────────────────────────────────────────────────

    /// @notice Returns true for IBurnMintERC20, IERC20, IERC165, and IAccessControl.
    function supportsInterface(bytes4 interfaceId)
        public
        view
        override(AccessControlUpgradeable)
        returns (bool)
    {
        return
            interfaceId == type(IBurnMintERC20).interfaceId ||
            interfaceId == type(IERC20).interfaceId ||
            super.supportsInterface(interfaceId);
    }
}
```

- [ ] **Step 4.2: Verify the contract compiles**

```bash
forge build
```

Expected: `Compiler run successful` with no errors.

- [ ] **Step 4.3: Run the deployment tests**

```bash
forge test --match-path "test/SyncUSD.t.sol" --match-test "test_decimals|test_name|test_admin|test_pauser|test_minter|test_notPaused|test_cannot" -v
```

Expected: All 9 deployment tests pass.

- [ ] **Step 4.4: Commit**

```bash
git add src/SyncUSD.sol test/SyncUSD.t.sol
git commit -m "feat(contracts): add SyncUSD skeleton with deployment tests"
```

---

## Chunk 3: Mint / Burn Tests + Implementation

### Task 5: Write and pass mint/burn tests

- [ ] **Step 5.1: Add mint/burn tests to `test/SyncUSD.t.sol`**

Add these test functions inside `SyncUSDTest`:

```solidity
    // ── Pre-mint gate ───────────────────────────────────────────────────

    function test_mintRevertsBeforeMinterRoleGranted() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        token.mint(user, 100e6);
    }

    // ── Mint ────────────────────────────────────────────────────────────

    function test_minterCanMint() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.prank(minter);
        token.mint(user, 100e6);

        assertEq(token.balanceOf(user), 100e6);
        assertEq(token.totalSupply(), 100e6);
    }

    function test_mintEmitsTransferEvent() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.expectEmit(true, true, false, true);
        emit Transfer(address(0), user, 100e6);

        vm.prank(minter);
        token.mint(user, 100e6);
    }

    function test_mintRevertsForUnauthorized() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.prank(unauthorized);
        vm.expectRevert();
        token.mint(user, 100e6);
    }

    // ── Burn ─────────────────────────────────────────────────────────────

    function test_minterCanBurnOwnBalance() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        // Mint to minter then burn
        vm.prank(minter);
        token.mint(minter, 200e6);
        assertEq(token.balanceOf(minter), 200e6);

        vm.prank(minter);
        token.burn(100e6);
        assertEq(token.balanceOf(minter), 100e6);
        assertEq(token.totalSupply(), 100e6);
    }

    function test_burnRevertsIfOverBalance() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.prank(minter);
        token.mint(minter, 50e6);

        vm.prank(minter);
        vm.expectRevert();
        token.burn(100e6);
    }

    function test_burnFromWithSufficientAllowance() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        // Give user some tokens
        vm.prank(minter);
        token.mint(user, 200e6);

        // User approves minter to burn on their behalf
        vm.prank(user);
        token.approve(minter, 100e6);

        vm.prank(minter);
        token.burnFrom(user, 100e6);

        assertEq(token.balanceOf(user), 100e6);
    }

    function test_burnFromRevertsIfAllowanceInsufficient() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.prank(minter);
        token.mint(user, 200e6);

        // No approval given
        vm.prank(minter);
        vm.expectRevert();
        token.burnFrom(user, 100e6);
    }
```

Also add the `Transfer` event declaration near the top of the test contract (needed for `vm.expectEmit`):

```solidity
    event Transfer(address indexed from, address indexed to, uint256 value);
```

- [ ] **Step 5.2: Run mint/burn tests**

```bash
forge test --match-path "test/SyncUSD.t.sol" --match-test "test_mint|test_burn" -v
```

Expected: All 8 mint/burn tests pass.

- [ ] **Step 5.3: Run full test suite to confirm no regressions**

```bash
forge test -v
```

Expected: All tests pass.

- [ ] **Step 5.4: Commit**

```bash
git add test/SyncUSD.t.sol
git commit -m "test(contracts): add mint/burn tests for SyncUSD"
```

---

## Chunk 4: Pause Tests + Access Control Tests

### Task 6: Write and pass pause tests

- [ ] **Step 6.1: Add pause tests to `test/SyncUSD.t.sol`**

Add these test functions inside `SyncUSDTest`:

```solidity
    // ── Pause ────────────────────────────────────────────────────────────

    function test_pauserCanPause() public {
        vm.prank(pauser);
        token.pause();
        assertTrue(token.paused());
    }

    function test_pauserCanUnpause() public {
        vm.prank(pauser);
        token.pause();

        vm.prank(pauser);
        token.unpause();
        assertFalse(token.paused());
    }

    function test_transferRevertsWhenPaused() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.prank(minter);
        token.mint(user, 100e6);

        vm.prank(pauser);
        token.pause();

        vm.prank(user);
        vm.expectRevert();
        token.transfer(unauthorized, 10e6);
    }

    function test_transferFromRevertsWhenPaused() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.prank(minter);
        token.mint(user, 100e6);

        vm.prank(user);
        token.approve(unauthorized, 50e6);

        vm.prank(pauser);
        token.pause();

        vm.prank(unauthorized);
        vm.expectRevert();
        token.transferFrom(user, unauthorized, 10e6);
    }

    function test_mintRevertsWhenPaused() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.prank(pauser);
        token.pause();

        vm.prank(minter);
        vm.expectRevert();
        token.mint(user, 100e6);
    }

    function test_burnRevertsWhenPaused() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.prank(minter);
        token.mint(minter, 100e6);

        vm.prank(pauser);
        token.pause();

        vm.prank(minter);
        vm.expectRevert();
        token.burn(50e6);
    }

    function test_burnFromRevertsWhenPaused() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.prank(minter);
        token.mint(user, 100e6);
        vm.prank(user);
        token.approve(minter, 50e6);

        vm.prank(pauser);
        token.pause();

        vm.prank(minter);
        vm.expectRevert();
        token.burnFrom(user, 50e6);
    }

    function test_transferWorksAfterUnpause() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.prank(minter);
        token.mint(user, 100e6);

        vm.prank(pauser);
        token.pause();
        vm.prank(pauser);
        token.unpause();

        vm.prank(user);
        token.transfer(unauthorized, 10e6);
        assertEq(token.balanceOf(unauthorized), 10e6);
    }

    // ── Access Control ───────────────────────────────────────────────────

    function test_adminCanGrantMinterRole() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        assertTrue(token.hasRole(token.MINTER_ROLE(), minter));
    }

    function test_adminCanRevokeMinterRole() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.prank(admin);
        token.revokeRole(token.MINTER_ROLE(), minter);
        assertFalse(token.hasRole(token.MINTER_ROLE(), minter));
    }

    function test_nonAdminCannotGrantRoles() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        token.grantRole(token.MINTER_ROLE(), unauthorized);
    }

    function test_mintRevertsAfterMinterRoleRevoked() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);

        vm.prank(admin);
        token.revokeRole(token.MINTER_ROLE(), minter);

        vm.prank(minter);
        vm.expectRevert();
        token.mint(user, 100e6);
    }
```

- [ ] **Step 6.2: Run pause and access control tests**

```bash
forge test --match-path "test/SyncUSD.t.sol" --match-test "test_pause|test_transfer|test_admin|test_nonAdmin|test_minterRole" -v
```

Expected: All pause and access control tests pass.

- [ ] **Step 6.3: Run full test suite**

```bash
forge test -v
```

Expected: All tests pass.

- [ ] **Step 6.4: Commit**

```bash
git add test/SyncUSD.t.sol
git commit -m "test(contracts): add pause and access control tests for SyncUSD"
```

---

## Chunk 5: Upgrade Tests

### Task 7: Write and pass UUPS upgrade tests

- [ ] **Step 7.1: Add upgrade tests to `test/SyncUSD.t.sol`**

First, add a V2 stub contract at the **top of the test file** (outside `SyncUSDTest`, after imports):

```solidity
/// @dev Minimal V2 used only to test upgrade. Inherits all V1 logic unchanged.
contract SyncUSDV2 is SyncUSD {
    function version() external pure returns (string memory) {
        return "v2";
    }
}
```

Then add these test functions inside `SyncUSDTest`:

```solidity
    // ── Upgrade ──────────────────────────────────────────────────────────

    function test_adminRoleCanUpgrade() public {
        // Mint some tokens first so we can verify storage is preserved
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.prank(minter);
        token.mint(user, 500e6);

        SyncUSDV2 newImpl = new SyncUSDV2();

        vm.prank(admin);
        token.upgradeToAndCall(address(newImpl), "");

        // Cast proxy to V2
        SyncUSDV2 tokenV2 = SyncUSDV2(address(token));
        assertEq(tokenV2.version(), "v2");
    }

    function test_upgradePreservesStorage() public {
        vm.prank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.prank(minter);
        token.mint(user, 500e6);

        SyncUSDV2 newImpl = new SyncUSDV2();
        vm.prank(admin);
        token.upgradeToAndCall(address(newImpl), "");

        // Balance must survive upgrade
        assertEq(token.balanceOf(user), 500e6);
        assertEq(token.decimals(), 6);
        assertEq(token.name(), "SyncUSD");
    }

    function test_upgradeRevertsForUnauthorized() public {
        SyncUSDV2 newImpl = new SyncUSDV2();

        vm.prank(unauthorized);
        vm.expectRevert();
        token.upgradeToAndCall(address(newImpl), "");
    }
```

- [ ] **Step 7.2: Run upgrade tests**

```bash
forge test --match-path "test/SyncUSD.t.sol" --match-test "test_admin|test_upgrade" -v
```

Expected: All upgrade tests pass.

- [ ] **Step 7.3: Run full test suite**

```bash
forge test -v
```

Expected: All tests pass.

- [ ] **Step 7.4: Commit**

```bash
git add test/SyncUSD.t.sol
git commit -m "test(contracts): add UUPS upgrade tests for SyncUSD"
```

---

## Chunk 6: CCIP supportsInterface Tests + Final Verification

### Task 8: Write and pass CCIP interface detection tests

- [ ] **Step 8.1: Add CCIP supportsInterface tests to `test/SyncUSD.t.sol`**

Add to `SyncUSDTest`:

```solidity
    // ── CCIP / ERC-165 ───────────────────────────────────────────────────

    function test_supportsIBurnMintERC20Interface() public view {
        bytes4 id = type(IBurnMintERC20).interfaceId;
        assertTrue(token.supportsInterface(id));
    }

    function test_supportsIERC20Interface() public view {
        bytes4 id = type(IERC20).interfaceId;
        assertTrue(token.supportsInterface(id));
    }

    function test_supportsIAccessControlInterface() public view {
        bytes4 id = type(IAccessControl).interfaceId;
        assertTrue(token.supportsInterface(id));
    }

    function test_doesNotSupportRandomInterface() public view {
        bytes4 id = bytes4(keccak256("randomInterface()"));
        assertFalse(token.supportsInterface(id));
    }
```

Also add the necessary imports at the top of the test file:

```solidity
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IAccessControl} from "@openzeppelin/contracts/access/IAccessControl.sol";
import {IBurnMintERC20} from "../src/interfaces/IBurnMintERC20.sol";
```

- [ ] **Step 8.2: Run CCIP tests**

```bash
forge test --match-path "test/SyncUSD.t.sol" --match-test "test_supports" -v
```

Expected: All 4 supportsInterface tests pass.

- [ ] **Step 8.3: Run the complete test suite**

```bash
forge test -v
```

Expected: All tests pass with no failures or compiler warnings.

- [ ] **Step 8.4: Run forge build to confirm clean compile**

```bash
forge build
```

Expected: `Compiler run successful` with no warnings.

- [ ] **Step 8.5: Commit**

```bash
git add test/SyncUSD.t.sol
git commit -m "test(contracts): add CCIP supportsInterface tests for SyncUSD"
```

---

### Task 9: Acceptance verification

- [ ] **Step 9.1: Final full test run**

```bash
forge test -v
```

Expected output (all green):
```
[PASS] test_decimalsIsSix()
[PASS] test_nameAndSymbol()
[PASS] test_adminHasDefaultAdminRole()
[PASS] test_adminHasAdminRole()
[PASS] test_pauserHasPauserRole()
[PASS] test_minterRoleNotGrantedAtInit()
[PASS] test_notPausedAfterInit()
[PASS] test_cannotReinitialize()
[PASS] test_implementationCannotBeInitialized()
[PASS] test_mintRevertsBeforeMinterRoleGranted()
[PASS] test_minterCanMint()
[PASS] test_mintEmitsTransferEvent()
[PASS] test_mintRevertsForUnauthorized()
[PASS] test_minterCanBurnOwnBalance()
[PASS] test_burnRevertsIfOverBalance()
[PASS] test_burnFromWithSufficientAllowance()
[PASS] test_burnFromRevertsIfAllowanceInsufficient()
[PASS] test_pauserCanPause()
[PASS] test_pauserCanUnpause()
[PASS] test_transferRevertsWhenPaused()
[PASS] test_transferFromRevertsWhenPaused()
[PASS] test_mintRevertsWhenPaused()
[PASS] test_burnRevertsWhenPaused()
[PASS] test_burnFromRevertsWhenPaused()
[PASS] test_transferWorksAfterUnpause()
[PASS] test_adminCanGrantMinterRole()
[PASS] test_adminCanRevokeMinterRole()
[PASS] test_nonAdminCannotGrantRoles()
[PASS] test_mintRevertsAfterMinterRoleRevoked()
[PASS] test_adminRoleCanUpgrade()
[PASS] test_upgradePreservesStorage()
[PASS] test_upgradeRevertsForUnauthorized()
[PASS] test_supportsIBurnMintERC20Interface()
[PASS] test_supportsIERC20Interface()
[PASS] test_supportsIAccessControlInterface()
[PASS] test_doesNotSupportRandomInterface()
```

- [ ] **Step 9.2: Final commit**

```bash
git add -A
git commit -m "feat(contracts): complete SyncUSD token contract implementation"
```
