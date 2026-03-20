// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console2} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {SyncUSD} from "../src/SyncUSD.sol";
import {Bank} from "../src/Bank.sol";

// ── Minimal USDC mock ─────────────────────────────────────────────────────────

contract MockUSDC is ERC20 {
    constructor() ERC20("USD Coin", "USDC") {}

    function decimals() public pure override returns (uint8) {
        return 6;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

// ── Integration test suite ────────────────────────────────────────────────────

/// @title BankIntegration
/// @notice Covers the full deposit → hot-path → release lifecycle plus the
///         pool-depletion and pause edge cases on a single Anvil fork.
///
///         Multi-chain nuance: in production, source and destination chains are
///         separate networks. Here we model both sides on one Anvil instance,
///         reusing a single SyncUSD + Bank pair, so we can exercise the
///         logic without needing two concurrent forks.
contract BankIntegration is Test {
    // ── Actors ────────────────────────────────────────────────────────

    address internal admin   = makeAddr("admin");
    address internal pauser  = makeAddr("pauser");
    address internal relayer = makeAddr("relayer");
    address internal alice   = makeAddr("alice");
    address internal bob     = makeAddr("bob");

    // ── Contracts ─────────────────────────────────────────────────────

    MockUSDC internal usdc;
    SyncUSD  internal syncUsd;
    Bank     internal bank;

    // ── Constants ─────────────────────────────────────────────────────

    uint256 internal constant DEPOSIT_AMOUNT = 1_000e6;   // 1 000 USDC

    bytes32 internal constant SOURCE_EVENT_1 = keccak256("source-event-1");
    bytes32 internal constant SOURCE_EVENT_2 = keccak256("source-event-2");

    // ── Setup ─────────────────────────────────────────────────────────

    function setUp() public {
        // Deploy mock USDC
        usdc = new MockUSDC();

        // Deploy SyncUSD proxy
        SyncUSD syncImpl = new SyncUSD();
        bytes memory syncInit = abi.encodeCall(SyncUSD.initialize, (admin, pauser));
        syncUsd = SyncUSD(address(new ERC1967Proxy(address(syncImpl), syncInit)));

        // Deploy Bank proxy
        Bank bankImpl = new Bank();
        bytes memory bankInit = abi.encodeCall(
            Bank.initialize,
            (admin, pauser, address(syncUsd), address(0))
        );
        bank = Bank(address(new ERC1967Proxy(address(bankImpl), bankInit)));

        // Pre-fetch role selectors before using vm.prank
        bytes32 minterRole  = syncUsd.MINTER_ROLE();
        bytes32 relayerRole = bank.RELAYER_ROLE();

        // Assign roles: Bank → MINTER_ROLE on SyncUSD
        vm.prank(admin);
        syncUsd.grantRole(minterRole, address(bank));

        // Assign roles: relayer → RELAYER_ROLE on Bank
        vm.prank(admin);
        bank.grantRole(relayerRole, relayer);

        // Seed alice with USDC
        usdc.mint(alice, DEPOSIT_AMOUNT * 10);
    }

    // ── Helpers ───────────────────────────────────────────────────────

    /// @dev Alice deposits `amount` USDC → receives SyncUSD.
    function _aliceDeposit(uint256 amount) internal {
        vm.startPrank(alice);
        usdc.approve(address(bank), amount);
        bank.deposit(address(usdc), amount);
        vm.stopPrank();
    }

    /// @dev Fund bank pool with SyncUSD by depositing and locking via transferHotPath.
    function _fundPool(uint256 amount) internal {
        _aliceDeposit(amount);
        vm.startPrank(alice);
        syncUsd.approve(address(bank), amount);
        bank.transferHotPath(alice, amount, 42);
        vm.stopPrank();
    }

    // ══════════════════════════════════════════════════════════════════
    // 1. Full lifecycle: deposit → SyncUSD → hot-path release
    // ══════════════════════════════════════════════════════════════════

    function test_fullLifecycle_depositMintsSyncUSD() public {
        uint256 aliceUsdcBefore = usdc.balanceOf(alice);

        _aliceDeposit(DEPOSIT_AMOUNT);

        // Alice's USDC leaves her wallet
        assertEq(usdc.balanceOf(alice), aliceUsdcBefore - DEPOSIT_AMOUNT);
        // Bank holds the USDC
        assertEq(usdc.balanceOf(address(bank)), DEPOSIT_AMOUNT);
        // Alice receives SyncUSD 1:1
        assertEq(syncUsd.balanceOf(alice), DEPOSIT_AMOUNT);
    }

    function test_fullLifecycle_depositEmitsEvent() public {
        vm.startPrank(alice);
        usdc.approve(address(bank), DEPOSIT_AMOUNT);

        vm.expectEmit(true, false, false, true);
        emit Bank.Deposited(alice, address(usdc), DEPOSIT_AMOUNT);

        bank.deposit(address(usdc), DEPOSIT_AMOUNT);
        vm.stopPrank();
    }

    function test_fullLifecycle_hotPathRelease() public {
        _fundPool(DEPOSIT_AMOUNT);

        uint256 poolBefore = bank.poolDepth();

        // Relayer releases SyncUSD to bob on the destination side
        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);

        assertEq(syncUsd.balanceOf(bob), DEPOSIT_AMOUNT, "bob did not receive SyncUSD");
        assertEq(bank.poolDepth(), poolBefore - DEPOSIT_AMOUNT, "pool not decremented");
        assertTrue(bank.released(SOURCE_EVENT_1), "transfer not marked released");
    }

    function test_fullLifecycle_emitsReleaseEvent() public {
        _fundPool(DEPOSIT_AMOUNT);

        vm.expectEmit(true, false, true, true);
        emit Bank.HotPathReleased(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);

        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);
    }

    function test_fullLifecycle_withdrawReturnsUSDC() public {
        _aliceDeposit(DEPOSIT_AMOUNT);

        vm.startPrank(alice);
        syncUsd.approve(address(bank), DEPOSIT_AMOUNT);
        bank.withdraw(address(usdc), DEPOSIT_AMOUNT);
        vm.stopPrank();

        assertEq(syncUsd.balanceOf(alice), 0);
        assertEq(usdc.balanceOf(alice), DEPOSIT_AMOUNT * 10); // back to starting
    }

    // ══════════════════════════════════════════════════════════════════
    // 2. Pool depletion: hot-path reverts when pool is empty
    // ══════════════════════════════════════════════════════════════════

    function test_poolDepletion_revertWhenEmpty() public {
        assertEq(bank.poolDepth(), 0);

        vm.prank(relayer);
        vm.expectRevert(Bank.InsufficientPoolLiquidity.selector);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);
    }

    function test_poolDepletion_revertAfterDrained() public {
        _fundPool(DEPOSIT_AMOUNT); // only enough for one release

        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);

        assertEq(bank.poolDepth(), 0);

        // Second release fails — pool drained
        vm.prank(relayer);
        vm.expectRevert(Bank.InsufficientPoolLiquidity.selector);
        bank.releaseHotPath(alice, DEPOSIT_AMOUNT, SOURCE_EVENT_2);
    }

    function test_poolDepletion_partialReleaseSucceeds() public {
        _fundPool(DEPOSIT_AMOUNT); // exactly 1 000 SyncUSD

        // Release half
        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT / 2, SOURCE_EVENT_1);

        assertEq(bank.poolDepth(), DEPOSIT_AMOUNT / 2);

        // Release the other half
        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT / 2, SOURCE_EVENT_2);

        assertEq(bank.poolDepth(), 0);
    }

    function test_poolDepletion_replayReverts() public {
        _fundPool(DEPOSIT_AMOUNT * 2);

        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);

        // Replaying the same sourceEventHash must revert
        vm.prank(relayer);
        vm.expectRevert(
            abi.encodeWithSelector(
                Bank.TransferAlreadyReleased.selector,
                SOURCE_EVENT_1
            )
        );
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);
    }

    // ══════════════════════════════════════════════════════════════════
    // 3. Pause scenario: watcher pauses, all mutations revert
    // ══════════════════════════════════════════════════════════════════

    function test_pause_blocksDeposit() public {
        vm.prank(pauser);
        bank.pause();

        vm.startPrank(alice);
        usdc.approve(address(bank), DEPOSIT_AMOUNT);
        vm.expectRevert();
        bank.deposit(address(usdc), DEPOSIT_AMOUNT);
        vm.stopPrank();
    }

    function test_pause_blocksHotPathRelease() public {
        _fundPool(DEPOSIT_AMOUNT);

        vm.prank(pauser);
        bank.pause();

        vm.prank(relayer);
        vm.expectRevert();
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);
    }

    function test_pause_blocksWithdraw() public {
        _aliceDeposit(DEPOSIT_AMOUNT);

        vm.prank(pauser);
        bank.pause();

        vm.startPrank(alice);
        syncUsd.approve(address(bank), DEPOSIT_AMOUNT);
        vm.expectRevert();
        bank.withdraw(address(usdc), DEPOSIT_AMOUNT);
        vm.stopPrank();
    }

    function test_pause_blocksTransferHotPath() public {
        _aliceDeposit(DEPOSIT_AMOUNT);

        vm.prank(pauser);
        bank.pause();

        vm.startPrank(alice);
        syncUsd.approve(address(bank), DEPOSIT_AMOUNT);
        vm.expectRevert();
        bank.transferHotPath(bob, DEPOSIT_AMOUNT, 42);
        vm.stopPrank();
    }

    function test_pause_syncUsdTransfersBlocked() public {
        _aliceDeposit(DEPOSIT_AMOUNT);

        vm.prank(pauser);
        syncUsd.pause();

        vm.prank(alice);
        vm.expectRevert();
        syncUsd.transfer(bob, DEPOSIT_AMOUNT);
    }

    function test_pause_unpauseRestoresDeposit() public {
        vm.prank(pauser);
        bank.pause();

        vm.prank(pauser);
        bank.unpause();

        _aliceDeposit(DEPOSIT_AMOUNT);
        assertEq(syncUsd.balanceOf(alice), DEPOSIT_AMOUNT);
    }

    function test_pause_unpauseRestoresHotPath() public {
        _fundPool(DEPOSIT_AMOUNT);

        vm.prank(pauser);
        bank.pause();

        vm.prank(pauser);
        bank.unpause();

        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);

        assertEq(syncUsd.balanceOf(bob), DEPOSIT_AMOUNT);
    }

    function test_pause_onlyPauserCanPause() public {
        vm.prank(alice);
        vm.expectRevert();
        bank.pause();
    }

    function test_pause_onlyPauserCanUnpause() public {
        vm.prank(pauser);
        bank.pause();

        vm.prank(alice);
        vm.expectRevert();
        bank.unpause();
    }

    // ══════════════════════════════════════════════════════════════════
    // 4. Access control
    // ══════════════════════════════════════════════════════════════════

    function test_acl_onlyRelayerCanRelease() public {
        _fundPool(DEPOSIT_AMOUNT);

        vm.prank(alice);
        vm.expectRevert();
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT, SOURCE_EVENT_1);
    }

    function test_acl_onlyMinterCanMintSyncUSD() public {
        vm.prank(alice);
        vm.expectRevert();
        syncUsd.mint(alice, DEPOSIT_AMOUNT);
    }

    // ══════════════════════════════════════════════════════════════════
    // 5. Zero-value guards
    // ══════════════════════════════════════════════════════════════════

    function test_zeroAmount_depositReverts() public {
        vm.prank(alice);
        vm.expectRevert(Bank.ZeroAmount.selector);
        bank.deposit(address(usdc), 0);
    }

    function test_zeroAmount_hotPathReleaseReverts() public {
        vm.prank(relayer);
        vm.expectRevert(Bank.ZeroAmount.selector);
        bank.releaseHotPath(bob, 0, SOURCE_EVENT_1);
    }

    function test_zeroAddress_hotPathReleaseReverts() public {
        _fundPool(DEPOSIT_AMOUNT);

        vm.prank(relayer);
        vm.expectRevert(Bank.ZeroAddress.selector);
        bank.releaseHotPath(address(0), DEPOSIT_AMOUNT, SOURCE_EVENT_1);
    }

    // ══════════════════════════════════════════════════════════════════
    // 6. poolDepth view
    // ══════════════════════════════════════════════════════════════════

    function test_poolDepth_reflectsBalance() public {
        assertEq(bank.poolDepth(), 0);

        _fundPool(DEPOSIT_AMOUNT);
        assertEq(bank.poolDepth(), DEPOSIT_AMOUNT);

        vm.prank(relayer);
        bank.releaseHotPath(bob, DEPOSIT_AMOUNT / 2, SOURCE_EVENT_1);
        assertEq(bank.poolDepth(), DEPOSIT_AMOUNT / 2);
    }

    // ══════════════════════════════════════════════════════════════════
    // 7. Upgrade authorization
    // ══════════════════════════════════════════════════════════════════

    function test_upgrade_onlyAdminCanUpgradeSyncUSD() public {
        SyncUSD newImpl = new SyncUSD();

        vm.prank(alice);
        vm.expectRevert();
        syncUsd.upgradeToAndCall(address(newImpl), "");

        vm.prank(admin);
        syncUsd.upgradeToAndCall(address(newImpl), "");
    }

    function test_upgrade_onlyAdminCanUpgradeBank() public {
        Bank newImpl = new Bank();

        vm.prank(alice);
        vm.expectRevert();
        bank.upgradeToAndCall(address(newImpl), "");

        vm.prank(admin);
        bank.upgradeToAndCall(address(newImpl), "");
    }
}
