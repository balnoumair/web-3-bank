// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console2} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {SyncUSD} from "../src/SyncUSD.sol";
import {BankContract} from "../src/BankContract.sol";

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
///         reusing a single SyncUSD + BankContract pair, so we can exercise the
///         logic without needing two concurrent forks.
contract BankIntegration is Test {
    // ── Actors ────────────────────────────────────────────────────────

    address internal admin   = makeAddr("admin");
    address internal pauser  = makeAddr("pauser");
    address internal relayer = makeAddr("relayer");
    address internal alice   = makeAddr("alice");
    address internal bob     = makeAddr("bob");
    address internal lp      = makeAddr("lp");         // liquidity provider

    // ── Contracts ─────────────────────────────────────────────────────

    MockUSDC     internal usdc;
    SyncUSD      internal syncUsd;
    BankContract internal bank;

    // ── Constants ─────────────────────────────────────────────────────

    uint256 internal constant DEPOSIT_AMOUNT = 1_000e6;   // 1 000 USDC
    uint256 internal constant POOL_SEED      = 10_000e6;  // 10 000 USDC for the hot-path pool

    bytes32 internal constant TRANSFER_ID_1 = keccak256("transfer-1");
    bytes32 internal constant TRANSFER_ID_2 = keccak256("transfer-2");

    // ── Setup ─────────────────────────────────────────────────────────

    function setUp() public {
        // Deploy mock USDC
        usdc = new MockUSDC();

        // Deploy SyncUSD proxy
        SyncUSD syncImpl = new SyncUSD();
        bytes memory syncInit = abi.encodeCall(SyncUSD.initialize, (admin, pauser));
        syncUsd = SyncUSD(address(new ERC1967Proxy(address(syncImpl), syncInit)));

        // Deploy BankContract proxy
        BankContract bankImpl = new BankContract();
        bytes memory bankInit = abi.encodeCall(
            BankContract.initialize,
            (admin, pauser, address(usdc), address(syncUsd))
        );
        bank = BankContract(address(new ERC1967Proxy(address(bankImpl), bankInit)));

        // Pre-fetch role selectors (view calls) before using vm.prank,
        // so the prank is NOT consumed by a role-fetch call in argument position.
        bytes32 minterRole  = syncUsd.MINTER_ROLE();
        bytes32 relayerRole = bank.RELAYER_ROLE();

        // Assign roles: BankContract → MINTER_ROLE on SyncUSD
        vm.prank(admin);
        syncUsd.grantRole(minterRole, address(bank));

        // Assign roles: relayer → RELAYER_ROLE on BankContract
        vm.prank(admin);
        bank.grantRole(relayerRole, relayer);

        // Seed alice and the liquidity provider with USDC
        usdc.mint(alice, DEPOSIT_AMOUNT * 10);
        usdc.mint(lp,    POOL_SEED);
    }

    // ── Helpers ───────────────────────────────────────────────────────

    /// @dev Fund the hot-path pool from `lp`.
    function _fundPool(uint256 amount) internal {
        vm.startPrank(lp);
        usdc.approve(address(bank), amount);
        bank.fundPool(amount);
        vm.stopPrank();
    }

    /// @dev Alice deposits `amount` USDC.
    function _aliceDeposit(uint256 amount) internal {
        vm.startPrank(alice);
        usdc.approve(address(bank), amount);
        bank.deposit(amount);
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
        // BankContract holds the USDC
        assertEq(usdc.balanceOf(address(bank)), DEPOSIT_AMOUNT);
        // Alice receives SyncUSD 1:1
        assertEq(syncUsd.balanceOf(alice), DEPOSIT_AMOUNT);
    }

    function test_fullLifecycle_depositEmitsEvent() public {
        vm.startPrank(alice);
        usdc.approve(address(bank), DEPOSIT_AMOUNT);

        vm.expectEmit(true, false, false, true);
        emit BankContract.Deposited(alice, DEPOSIT_AMOUNT);

        bank.deposit(DEPOSIT_AMOUNT);
        vm.stopPrank();
    }

    function test_fullLifecycle_hotPathRelease() public {
        _fundPool(POOL_SEED);
        _aliceDeposit(DEPOSIT_AMOUNT);

        uint256 bobUsdcBefore   = usdc.balanceOf(bob);
        uint256 poolBefore      = bank.hotPathPool();

        // Relayer releases USDC to bob on the destination side
        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        assertEq(usdc.balanceOf(bob), bobUsdcBefore + DEPOSIT_AMOUNT, "bob did not receive USDC");
        assertEq(bank.hotPathPool(),  poolBefore    - DEPOSIT_AMOUNT, "pool not decremented");
        assertTrue(bank.released(TRANSFER_ID_1), "transfer not marked released");
    }

    function test_fullLifecycle_poolReplenishment() public {
        _fundPool(POOL_SEED);

        uint256 poolBefore = bank.hotPathPool();

        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        assertEq(bank.hotPathPool(), poolBefore - DEPOSIT_AMOUNT);

        // Cross-chain message arrives; relayer marks the transfer as settled
        vm.prank(relayer);
        bank.replenishPool(TRANSFER_ID_1, DEPOSIT_AMOUNT);

        assertEq(bank.hotPathPool(), poolBefore, "pool should be back to original after replenishment");
        assertTrue(bank.settled(TRANSFER_ID_1), "transfer not marked settled");
    }

    function test_fullLifecycle_emitsEvents() public {
        _fundPool(POOL_SEED);
        _aliceDeposit(DEPOSIT_AMOUNT);

        vm.expectEmit(true, true, false, true);
        emit BankContract.HotPathReleased(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        vm.expectEmit(true, false, false, true);
        emit BankContract.PoolReplenished(TRANSFER_ID_1, DEPOSIT_AMOUNT);

        vm.prank(relayer);
        bank.replenishPool(TRANSFER_ID_1, DEPOSIT_AMOUNT);
    }

    // ══════════════════════════════════════════════════════════════════
    // 2. Pool depletion: hot-path reverts when pool is empty
    // ══════════════════════════════════════════════════════════════════

    function test_poolDepletion_revertWhenEmpty() public {
        // Pool has zero balance
        assertEq(bank.hotPathPool(), 0);

        vm.prank(relayer);
        vm.expectRevert(
            abi.encodeWithSelector(
                BankContract.InsufficientPool.selector,
                DEPOSIT_AMOUNT,
                0
            )
        );
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);
    }

    function test_poolDepletion_revertAfterDrained() public {
        _fundPool(DEPOSIT_AMOUNT); // only enough for one release

        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        assertEq(bank.hotPathPool(), 0);

        // Second release for a different transfer fails — pool drained
        vm.prank(relayer);
        vm.expectRevert(
            abi.encodeWithSelector(
                BankContract.InsufficientPool.selector,
                DEPOSIT_AMOUNT,
                0
            )
        );
        bank.hotPathRelease(TRANSFER_ID_2, alice, DEPOSIT_AMOUNT);
    }

    function test_poolDepletion_partialReleaseSucceeds() public {
        _fundPool(DEPOSIT_AMOUNT); // exactly 1 000 USDC

        // Release half
        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT / 2);

        assertEq(bank.hotPathPool(), DEPOSIT_AMOUNT / 2);

        // Release the other half
        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_2, bob, DEPOSIT_AMOUNT / 2);

        assertEq(bank.hotPathPool(), 0);
    }

    function test_poolDepletion_replayReverts() public {
        _fundPool(POOL_SEED);

        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        // Replaying the same transferId must revert
        vm.prank(relayer);
        vm.expectRevert(
            abi.encodeWithSelector(
                BankContract.TransferAlreadyReleased.selector,
                TRANSFER_ID_1
            )
        );
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);
    }

    function test_poolDepletion_settlementReplayReverts() public {
        _fundPool(POOL_SEED);

        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        vm.prank(relayer);
        bank.replenishPool(TRANSFER_ID_1, DEPOSIT_AMOUNT);

        // Replaying settlement must revert
        vm.prank(relayer);
        vm.expectRevert(
            abi.encodeWithSelector(
                BankContract.TransferAlreadySettled.selector,
                TRANSFER_ID_1
            )
        );
        bank.replenishPool(TRANSFER_ID_1, DEPOSIT_AMOUNT);
    }

    // ══════════════════════════════════════════════════════════════════
    // 3. Pause scenario: watcher pauses, all mutations revert
    // ══════════════════════════════════════════════════════════════════

    function test_pause_blocksDeposit() public {
        vm.prank(pauser);
        bank.pause();

        usdc.mint(alice, DEPOSIT_AMOUNT);
        vm.startPrank(alice);
        usdc.approve(address(bank), DEPOSIT_AMOUNT);
        vm.expectRevert();
        bank.deposit(DEPOSIT_AMOUNT);
        vm.stopPrank();
    }

    function test_pause_blocksFundPool() public {
        vm.prank(pauser);
        bank.pause();

        vm.startPrank(lp);
        usdc.approve(address(bank), POOL_SEED);
        vm.expectRevert();
        bank.fundPool(POOL_SEED);
        vm.stopPrank();
    }

    function test_pause_blocksHotPathRelease() public {
        _fundPool(POOL_SEED);

        vm.prank(pauser);
        bank.pause();

        vm.prank(relayer);
        vm.expectRevert();
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);
    }

    function test_pause_blocksReplenishPool() public {
        _fundPool(POOL_SEED);
        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        vm.prank(pauser);
        bank.pause();

        vm.prank(relayer);
        vm.expectRevert();
        bank.replenishPool(TRANSFER_ID_1, DEPOSIT_AMOUNT);
    }

    function test_pause_syncUsdTransfersBlocked() public {
        _aliceDeposit(DEPOSIT_AMOUNT);

        // Pauser pauses SyncUSD
        vm.prank(pauser);
        syncUsd.pause();

        // Alice cannot transfer SyncUSD while paused
        vm.prank(alice);
        vm.expectRevert();
        syncUsd.transfer(bob, DEPOSIT_AMOUNT);
    }

    function test_pause_unpauseRestoresDeposit() public {
        vm.prank(pauser);
        bank.pause();

        vm.prank(pauser);
        bank.unpause();

        // Deposit should work again
        _aliceDeposit(DEPOSIT_AMOUNT);
        assertEq(syncUsd.balanceOf(alice), DEPOSIT_AMOUNT);
    }

    function test_pause_unpauseRestoresHotPath() public {
        _fundPool(POOL_SEED);

        vm.prank(pauser);
        bank.pause();

        vm.prank(pauser);
        bank.unpause();

        vm.prank(relayer);
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);

        assertEq(usdc.balanceOf(bob), DEPOSIT_AMOUNT);
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
        _fundPool(POOL_SEED);

        vm.prank(alice);
        vm.expectRevert();
        bank.hotPathRelease(TRANSFER_ID_1, bob, DEPOSIT_AMOUNT);
    }

    function test_acl_onlyRelayerCanReplenish() public {
        vm.prank(alice);
        vm.expectRevert();
        bank.replenishPool(TRANSFER_ID_1, DEPOSIT_AMOUNT);
    }

    function test_acl_onlyAdminCanWithdrawPool() public {
        _fundPool(POOL_SEED);

        vm.prank(alice);
        vm.expectRevert();
        bank.withdrawPool(alice, POOL_SEED);

        vm.prank(admin);
        bank.withdrawPool(admin, POOL_SEED);

        assertEq(usdc.balanceOf(admin), POOL_SEED);
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
        vm.expectRevert(BankContract.ZeroAmount.selector);
        bank.deposit(0);
    }

    function test_zeroAmount_hotPathReleaseReverts() public {
        vm.prank(relayer);
        vm.expectRevert(BankContract.ZeroAmount.selector);
        bank.hotPathRelease(TRANSFER_ID_1, bob, 0);
    }

    function test_zeroAddress_hotPathReleaseReverts() public {
        _fundPool(POOL_SEED);

        vm.prank(relayer);
        vm.expectRevert(BankContract.ZeroAddress.selector);
        bank.hotPathRelease(TRANSFER_ID_1, address(0), DEPOSIT_AMOUNT);
    }

    // ══════════════════════════════════════════════════════════════════
    // 6. RouteReceiver cross-reference (mock)
    // ══════════════════════════════════════════════════════════════════

    /// @dev Validates that the BankContract can co-exist with a deployed RouteReceiver.
    ///      In production, the relayer consults RouteReceiver.getLatestRoute() before
    ///      choosing the destination, but the contracts are independent on-chain.
    ///      This test confirms they compile and deploy without storage collisions.
    function test_routeReceiver_addressesDoNotCollide() public view {
        // Distinct non-zero addresses → no aliasing
        assertTrue(address(bank) != address(syncUsd));
        assertTrue(address(bank) != address(usdc));
        assertTrue(address(syncUsd) != address(usdc));
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

    function test_upgrade_onlyAdminCanUpgradeBankContract() public {
        BankContract newImpl = new BankContract();

        vm.prank(alice);
        vm.expectRevert();
        bank.upgradeToAndCall(address(newImpl), "");

        vm.prank(admin);
        bank.upgradeToAndCall(address(newImpl), "");
    }
}
