// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SyncUSD} from "../src/SyncUSD.sol";
import {Bank} from "../src/Bank.sol";

/// @dev Minimal ERC-20 used as mock USDC in tests.
contract MockUSDC is ERC20 {
    constructor() ERC20("USD Coin", "USDC") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function decimals() public pure override returns (uint8) {
        return 6;
    }
}

/// @dev Minimal V2 used only to test UUPS upgrade.
contract BankV2 is Bank {
    function version() external pure returns (string memory) {
        return "v2";
    }
}

contract BankTest is Test {
    // ── Actors ─────────────────────────────────────────────────────────

    address public admin = address(0xA1);
    address public pauser = address(0xA2);
    address public relayer = address(0xA3);
    address public user = address(0xA4);
    address public recipient = address(0xA5);
    address public unauthorized = address(0xA6);
    address public feeCollector = address(0xA7);

    // ── Contracts ──────────────────────────────────────────────────────

    SyncUSD public syncUSD;
    Bank public bank;
    MockUSDC public usdc;

    // ── Cached roles ───────────────────────────────────────────────────

    bytes32 public MINTER_ROLE;
    bytes32 public RELAYER_ROLE;
    bytes32 public PAUSER_ROLE;
    bytes32 public ADMIN_ROLE;

    // ── Events (redeclared for expectEmit) ─────────────────────────────

    event Deposited(address indexed user, address underlyingToken, uint256 amount);
    event Withdrawn(address indexed user, address underlyingToken, uint256 amount);
    event HotPathInitiated(
        address indexed sender,
        address indexed to,
        uint256 amount,
        uint256 destinationChainId,
        bytes32 eventHash,
        uint256 fee
    );
    event HotPathReleased(address indexed to, uint256 amount, bytes32 indexed sourceEventHash);
    event FeeCollectorUpdated(address indexed feeCollector);

    // ── Setup ──────────────────────────────────────────────────────────

    function setUp() public {
        // Deploy SyncUSD proxy
        SyncUSD syncImpl = new SyncUSD();
        bytes memory syncInit = abi.encodeCall(SyncUSD.initialize, (admin, pauser));
        ERC1967Proxy syncProxy = new ERC1967Proxy(address(syncImpl), syncInit);
        syncUSD = SyncUSD(address(syncProxy));

        // Deploy Bank proxy
        Bank bankImpl = new Bank();
        bytes memory bankInit =
            abi.encodeCall(Bank.initialize, (admin, pauser, address(syncUSD), feeCollector));
        ERC1967Proxy bankProxy = new ERC1967Proxy(address(bankImpl), bankInit);
        bank = Bank(address(bankProxy));

        // Grant MINTER_ROLE on SyncUSD to the Bank
        MINTER_ROLE = syncUSD.MINTER_ROLE();
        vm.prank(admin);
        syncUSD.grantRole(MINTER_ROLE, address(bank));

        // Grant RELAYER_ROLE on Bank to the relayer
        RELAYER_ROLE = bank.RELAYER_ROLE();
        vm.prank(admin);
        bank.grantRole(RELAYER_ROLE, relayer);

        PAUSER_ROLE = bank.PAUSER_ROLE();
        ADMIN_ROLE = bank.ADMIN_ROLE();

        // Deploy mock USDC and fund the user
        usdc = new MockUSDC();
        usdc.mint(user, 10_000e6);
    }

    // ── Deployment ──────────────────────────────────────────────────────

    function test_syncUSDSet() public view {
        assertEq(address(bank.syncUSD()), address(syncUSD));
    }

    function test_feeCollectorSet() public view {
        assertEq(bank.feeCollector(), feeCollector);
    }

    function test_adminHasDefaultAdminRole() public view {
        assertTrue(bank.hasRole(bank.DEFAULT_ADMIN_ROLE(), admin));
    }

    function test_adminHasAdminRole() public view {
        assertTrue(bank.hasRole(ADMIN_ROLE, admin));
    }

    function test_pauserHasPauserRole() public view {
        assertTrue(bank.hasRole(PAUSER_ROLE, pauser));
    }

    function test_relayerHasRelayerRole() public view {
        assertTrue(bank.hasRole(RELAYER_ROLE, relayer));
    }

    function test_notPausedAfterInit() public view {
        assertFalse(bank.paused());
    }

    function test_cannotReinitialize() public {
        vm.expectRevert();
        bank.initialize(admin, pauser, address(syncUSD), feeCollector);
    }

    function test_initializeRevertsOnZeroAdmin() public {
        Bank impl = new Bank();
        bytes memory data =
            abi.encodeCall(Bank.initialize, (address(0), pauser, address(syncUSD), feeCollector));
        vm.expectRevert(Bank.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), data);
    }

    function test_initializeRevertsOnZeroPauser() public {
        Bank impl = new Bank();
        bytes memory data =
            abi.encodeCall(Bank.initialize, (admin, address(0), address(syncUSD), feeCollector));
        vm.expectRevert(Bank.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), data);
    }

    function test_initializeRevertsOnZeroSyncUSD() public {
        Bank impl = new Bank();
        bytes memory data =
            abi.encodeCall(Bank.initialize, (admin, pauser, address(0), feeCollector));
        vm.expectRevert(Bank.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), data);
    }

    // ── Deposit ─────────────────────────────────────────────────────────

    function test_depositEscrowsUSDCAndMintsSyncUSD() public {
        uint256 amount = 500e6;
        vm.startPrank(user);
        usdc.approve(address(bank), amount);
        bank.deposit(address(usdc), amount);
        vm.stopPrank();

        assertEq(usdc.balanceOf(address(bank)), amount);
        assertEq(syncUSD.balanceOf(user), amount);
    }

    function test_depositEmitsEvent() public {
        uint256 amount = 500e6;
        vm.startPrank(user);
        usdc.approve(address(bank), amount);

        vm.expectEmit(true, false, false, true);
        emit Deposited(user, address(usdc), amount);

        bank.deposit(address(usdc), amount);
        vm.stopPrank();
    }

    function test_depositRevertsOnZeroAmount() public {
        vm.prank(user);
        vm.expectRevert(Bank.ZeroAmount.selector);
        bank.deposit(address(usdc), 0);
    }

    function test_depositRevertsWithoutApproval() public {
        vm.prank(user);
        vm.expectRevert();
        bank.deposit(address(usdc), 100e6);
    }

    // ── Withdraw ────────────────────────────────────────────────────────

    function _depositForUser(uint256 amount) internal {
        vm.startPrank(user);
        usdc.approve(address(bank), amount);
        bank.deposit(address(usdc), amount);
        vm.stopPrank();
    }

    function test_withdrawBurnsSyncUSDAndReleasesUSDC() public {
        uint256 amount = 300e6;
        _depositForUser(amount);

        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.withdraw(address(usdc), amount);
        vm.stopPrank();

        assertEq(syncUSD.balanceOf(user), 0);
        assertEq(syncUSD.totalSupply(), 0);
        assertEq(usdc.balanceOf(user), 10_000e6); // back to starting balance
    }

    function test_withdrawEmitsEvent() public {
        uint256 amount = 200e6;
        _depositForUser(amount);

        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);

        vm.expectEmit(true, false, false, true);
        emit Withdrawn(user, address(usdc), amount);

        bank.withdraw(address(usdc), amount);
        vm.stopPrank();
    }

    function test_withdrawRevertsOnZeroAmount() public {
        vm.prank(user);
        vm.expectRevert(Bank.ZeroAmount.selector);
        bank.withdraw(address(usdc), 0);
    }

    function test_withdrawRevertsWithoutSyncUSDApproval() public {
        _depositForUser(100e6);

        vm.prank(user);
        vm.expectRevert();
        bank.withdraw(address(usdc), 100e6);
    }

    // ── Deposit → Withdraw round-trip ───────────────────────────────────

    function test_depositWithdrawRoundTrip() public {
        uint256 amount = 1_000e6;
        _depositForUser(amount);
        assertEq(syncUSD.balanceOf(user), amount);

        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.withdraw(address(usdc), amount);
        vm.stopPrank();

        assertEq(syncUSD.balanceOf(user), 0);
        assertEq(usdc.balanceOf(user), 10_000e6);
        assertEq(usdc.balanceOf(address(bank)), 0);
    }

    // ── Hot path ────────────────────────────────────────────────────────

    function _giveUserSyncUSD(uint256 amount) internal {
        _depositForUser(amount);
    }

    function test_transferHotPathLocksAndEmits() public {
        uint256 amount = 400e6;
        _giveUserSyncUSD(amount);

        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 42);
        vm.stopPrank();

        // SyncUSD moved from user to bank pool
        assertEq(syncUSD.balanceOf(user), 0);
        assertEq(syncUSD.balanceOf(address(bank)), amount);
    }

    function test_transferHotPathEmitsEvent() public {
        uint256 amount = 200e6;
        _giveUserSyncUSD(amount);

        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);

        // Only verify indexed and non-hash fields; hash is opaque
        vm.expectEmit(true, true, false, false);
        emit HotPathInitiated(user, recipient, amount, 42, bytes32(0), 0);

        bank.transferHotPath(recipient, amount, 42);
        vm.stopPrank();
    }

    function test_releaseHotPathTransfersSyncUSDToRecipient() public {
        // Fund the bank pool directly by having user do a hot path transfer
        uint256 amount = 300e6;
        _giveUserSyncUSD(amount);
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 42);
        vm.stopPrank();

        bytes32 sourceHash = keccak256("some-event-hash");
        vm.prank(relayer);
        bank.releaseHotPath(recipient, amount, sourceHash);

        assertEq(syncUSD.balanceOf(recipient), amount);
        assertEq(syncUSD.balanceOf(address(bank)), 0);
    }

    function test_releaseHotPathEmitsEvent() public {
        uint256 amount = 100e6;
        _giveUserSyncUSD(amount);
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 1);
        vm.stopPrank();

        bytes32 sourceHash = keccak256("source-event");
        vm.expectEmit(true, false, true, true);
        emit HotPathReleased(recipient, amount, sourceHash);

        vm.prank(relayer);
        bank.releaseHotPath(recipient, amount, sourceHash);
    }

    function test_hotPathInitiateReleaseFullFlow() public {
        uint256 amount = 500e6;
        _giveUserSyncUSD(amount);

        // Initiate on "source" chain
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 137);
        vm.stopPrank();

        assertEq(syncUSD.balanceOf(address(bank)), amount);

        // Release on "destination" chain (same bank in this test)
        bytes32 sourceHash = bytes32(uint256(1));
        vm.prank(relayer);
        bank.releaseHotPath(recipient, amount, sourceHash);

        assertEq(syncUSD.balanceOf(recipient), amount);
        assertEq(syncUSD.balanceOf(address(bank)), 0);
    }

    // ── Access control ──────────────────────────────────────────────────

    function test_releaseHotPathRevertsForNonRelayer() public {
        uint256 amount = 100e6;
        _giveUserSyncUSD(amount);
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 1);
        vm.stopPrank();

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.releaseHotPath(recipient, amount, bytes32(0));
    }

    function test_releaseHotPathRevertsForUser() public {
        uint256 amount = 100e6;
        _giveUserSyncUSD(amount);
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 1);
        vm.stopPrank();

        vm.prank(user);
        vm.expectRevert();
        bank.releaseHotPath(recipient, amount, bytes32(0));
    }

    function test_adminCanGrantRelayerRole() public {
        address newRelayer = address(0xBB);
        vm.prank(admin);
        bank.grantRole(RELAYER_ROLE, newRelayer);
        assertTrue(bank.hasRole(RELAYER_ROLE, newRelayer));
    }

    function test_adminCanRevokeRelayerRole() public {
        vm.prank(admin);
        bank.revokeRole(RELAYER_ROLE, relayer);
        assertFalse(bank.hasRole(RELAYER_ROLE, relayer));
    }

    function test_nonAdminCannotGrantRoles() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.grantRole(RELAYER_ROLE, unauthorized);
    }

    // ── Insufficient pool liquidity ─────────────────────────────────────

    function test_releaseHotPathRevertsOnInsufficientLiquidity() public {
        // Pool has 100e6, trying to release 200e6
        uint256 poolFund = 100e6;
        _giveUserSyncUSD(poolFund);
        vm.startPrank(user);
        syncUSD.approve(address(bank), poolFund);
        bank.transferHotPath(recipient, poolFund, 1);
        vm.stopPrank();

        vm.prank(relayer);
        vm.expectRevert(Bank.InsufficientPoolLiquidity.selector);
        bank.releaseHotPath(recipient, 200e6, bytes32(0));
    }

    function test_releaseHotPathRevertsOnEmptyPool() public {
        // Pool has no SyncUSD
        vm.prank(relayer);
        vm.expectRevert(Bank.InsufficientPoolLiquidity.selector);
        bank.releaseHotPath(recipient, 1e6, bytes32(0));
    }

    // ── ZeroAddress guards ──────────────────────────────────────────────

    function test_transferHotPathRevertsOnZeroRecipient() public {
        _giveUserSyncUSD(100e6);
        vm.startPrank(user);
        syncUSD.approve(address(bank), 100e6);
        vm.expectRevert(Bank.ZeroAddress.selector);
        bank.transferHotPath(address(0), 100e6, 1);
        vm.stopPrank();
    }

    function test_releaseHotPathRevertsOnZeroRecipient() public {
        uint256 amount = 100e6;
        _giveUserSyncUSD(amount);
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 1);
        vm.stopPrank();

        vm.prank(relayer);
        vm.expectRevert(Bank.ZeroAddress.selector);
        bank.releaseHotPath(address(0), amount, bytes32(0));
    }

    // ── Pause ───────────────────────────────────────────────────────────

    function test_pauserCanPause() public {
        vm.prank(pauser);
        bank.pause();
        assertTrue(bank.paused());
    }

    function test_pauserCanUnpause() public {
        vm.prank(pauser);
        bank.pause();
        vm.prank(pauser);
        bank.unpause();
        assertFalse(bank.paused());
    }

    function test_pauseRevertsForUnauthorized() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.pause();
    }

    function test_unpauseRevertsForUnauthorized() public {
        vm.prank(pauser);
        bank.pause();
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.unpause();
    }

    function test_depositRevertsWhenPaused() public {
        vm.prank(pauser);
        bank.pause();

        vm.startPrank(user);
        usdc.approve(address(bank), 100e6);
        vm.expectRevert();
        bank.deposit(address(usdc), 100e6);
        vm.stopPrank();
    }

    function test_withdrawRevertsWhenPaused() public {
        _depositForUser(100e6);
        vm.prank(pauser);
        bank.pause();

        vm.startPrank(user);
        syncUSD.approve(address(bank), 100e6);
        vm.expectRevert();
        bank.withdraw(address(usdc), 100e6);
        vm.stopPrank();
    }

    function test_transferHotPathRevertsWhenPaused() public {
        _giveUserSyncUSD(100e6);
        vm.prank(pauser);
        bank.pause();

        vm.startPrank(user);
        syncUSD.approve(address(bank), 100e6);
        vm.expectRevert();
        bank.transferHotPath(recipient, 100e6, 1);
        vm.stopPrank();
    }

    function test_releaseHotPathRevertsWhenPaused() public {
        uint256 amount = 100e6;
        _giveUserSyncUSD(amount);
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 1);
        vm.stopPrank();

        vm.prank(pauser);
        bank.pause();

        vm.prank(relayer);
        vm.expectRevert();
        bank.releaseHotPath(recipient, amount, bytes32(0));
    }

    function test_depositWorksAfterUnpause() public {
        vm.prank(pauser);
        bank.pause();
        vm.prank(pauser);
        bank.unpause();

        vm.startPrank(user);
        usdc.approve(address(bank), 100e6);
        bank.deposit(address(usdc), 100e6);
        vm.stopPrank();

        assertEq(syncUSD.balanceOf(user), 100e6);
    }

    // ── Fee collector ───────────────────────────────────────────────────

    function test_adminCanSetFeeCollector() public {
        address newCollector = address(0xFEE);
        vm.prank(admin);
        bank.setFeeCollector(newCollector);
        assertEq(bank.feeCollector(), newCollector);
    }

    function test_setFeeCollectorEmitsEvent() public {
        address newCollector = address(0xFEE);
        vm.expectEmit(true, false, false, false);
        emit FeeCollectorUpdated(newCollector);
        vm.prank(admin);
        bank.setFeeCollector(newCollector);
    }

    function test_nonAdminCannotSetFeeCollector() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setFeeCollector(address(0xFEE));
    }

    // ── UUPS upgrade ────────────────────────────────────────────────────

    function test_adminCanUpgrade() public {
        BankV2 newImpl = new BankV2();
        vm.prank(admin);
        bank.upgradeToAndCall(address(newImpl), "");

        BankV2 bankV2 = BankV2(address(bank));
        assertEq(bankV2.version(), "v2");
    }

    function test_upgradePreservesStorage() public {
        BankV2 newImpl = new BankV2();
        vm.prank(admin);
        bank.upgradeToAndCall(address(newImpl), "");

        assertEq(address(bank.syncUSD()), address(syncUSD));
        assertEq(bank.feeCollector(), feeCollector);
        assertTrue(bank.hasRole(RELAYER_ROLE, relayer));
    }

    function test_upgradeRevertsForUnauthorized() public {
        BankV2 newImpl = new BankV2();
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.upgradeToAndCall(address(newImpl), "");
    }
}
