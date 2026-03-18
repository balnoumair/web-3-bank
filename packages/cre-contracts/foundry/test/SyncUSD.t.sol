// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IAccessControl} from "@openzeppelin/contracts/access/IAccessControl.sol";
import {IBurnMintERC20} from "../src/interfaces/IBurnMintERC20.sol";
import {SyncUSD} from "../src/SyncUSD.sol";

/// @dev Minimal V2 used only to test upgrade. Inherits all V1 logic unchanged.
contract SyncUSDV2 is SyncUSD {
    function version() external pure returns (string memory) {
        return "v2";
    }
}

contract SyncUSDTest is Test {
    SyncUSD public implementation;
    SyncUSD public token; // proxy cast to SyncUSD

    address public admin = address(0xA1);
    address public pauser = address(0xA2);
    address public minter = address(0xA3);
    address public user = address(0xA4);
    address public unauthorized = address(0xA5);

    // Cached role constants — avoids consuming vm.prank with an extra external call
    bytes32 public MINTER_ROLE;

    event Transfer(address indexed from, address indexed to, uint256 value);

    function setUp() public {
        implementation = new SyncUSD();
        bytes memory initData = abi.encodeCall(SyncUSD.initialize, (admin, pauser));
        ERC1967Proxy proxy = new ERC1967Proxy(address(implementation), initData);
        token = SyncUSD(address(proxy));
        MINTER_ROLE = token.MINTER_ROLE();
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
        bytes32 MINTER_ROLE_ = keccak256("MINTER_ROLE");
        assertFalse(token.hasRole(MINTER_ROLE_, admin));
        assertFalse(token.hasRole(MINTER_ROLE_, pauser));
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

    function test_initializeRevertsOnZeroAdmin() public {
        SyncUSD impl = new SyncUSD();
        bytes memory initData = abi.encodeCall(SyncUSD.initialize, (address(0), pauser));
        vm.expectRevert(SyncUSD.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), initData);
    }

    function test_initializeRevertsOnZeroPauser() public {
        SyncUSD impl = new SyncUSD();
        bytes memory initData = abi.encodeCall(SyncUSD.initialize, (admin, address(0)));
        vm.expectRevert(SyncUSD.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), initData);
    }

    // ── Pre-mint gate ───────────────────────────────────────────────────

    function test_mintRevertsBeforeMinterRoleGranted() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        token.mint(user, 100e6);
    }

    // ── Mint ────────────────────────────────────────────────────────────

    function test_minterCanMint() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(minter);
        token.mint(user, 100e6);

        assertEq(token.balanceOf(user), 100e6);
        assertEq(token.totalSupply(), 100e6);
    }

    function test_mintEmitsTransferEvent() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

        vm.expectEmit(true, true, false, true);
        emit Transfer(address(0), user, 100e6);

        vm.prank(minter);
        token.mint(user, 100e6);
    }

    function test_mintRevertsForUnauthorized() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(unauthorized);
        vm.expectRevert();
        token.mint(user, 100e6);
    }

    // ── Burn ─────────────────────────────────────────────────────────────

    function test_minterCanBurnOwnBalance() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

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
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(minter);
        token.mint(minter, 50e6);

        vm.prank(minter);
        vm.expectRevert();
        token.burn(100e6);
    }

    function test_burnFromWithSufficientAllowance() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(minter);
        token.mint(user, 200e6);

        vm.prank(user);
        token.approve(minter, 100e6);

        vm.prank(minter);
        token.burnFrom(user, 100e6);

        assertEq(token.balanceOf(user), 100e6);
    }

    function test_burnFromRevertsIfAllowanceInsufficient() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(minter);
        token.mint(user, 200e6);

        vm.prank(minter);
        vm.expectRevert();
        token.burnFrom(user, 100e6);
    }

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
        token.grantRole(MINTER_ROLE, minter);
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
        token.grantRole(MINTER_ROLE, minter);
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
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(pauser);
        token.pause();

        vm.prank(minter);
        vm.expectRevert();
        token.mint(user, 100e6);
    }

    function test_burnRevertsWhenPaused() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);
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
        token.grantRole(MINTER_ROLE, minter);
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
        token.grantRole(MINTER_ROLE, minter);
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
        token.grantRole(MINTER_ROLE, minter);
        assertTrue(token.hasRole(MINTER_ROLE, minter));
    }

    function test_adminCanRevokeMinterRole() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(admin);
        token.revokeRole(MINTER_ROLE, minter);
        assertFalse(token.hasRole(MINTER_ROLE, minter));
    }

    function test_nonAdminCannotGrantRoles() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        token.grantRole(MINTER_ROLE, unauthorized);
    }

    function test_mintRevertsAfterMinterRoleRevoked() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);

        vm.prank(admin);
        token.revokeRole(MINTER_ROLE, minter);

        vm.prank(minter);
        vm.expectRevert();
        token.mint(user, 100e6);
    }

    // ── Upgrade ──────────────────────────────────────────────────────────

    function test_adminRoleCanUpgrade() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);
        vm.prank(minter);
        token.mint(user, 500e6);

        SyncUSDV2 newImpl = new SyncUSDV2();

        vm.prank(admin);
        token.upgradeToAndCall(address(newImpl), "");

        SyncUSDV2 tokenV2 = SyncUSDV2(address(token));
        assertEq(tokenV2.version(), "v2");
    }

    function test_upgradePreservesStorage() public {
        vm.prank(admin);
        token.grantRole(MINTER_ROLE, minter);
        vm.prank(minter);
        token.mint(user, 500e6);

        SyncUSDV2 newImpl = new SyncUSDV2();
        vm.prank(admin);
        token.upgradeToAndCall(address(newImpl), "");

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
}
