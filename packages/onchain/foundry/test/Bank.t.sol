// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SyncUSD} from "../src/SyncUSD.sol";
import {Bank} from "../src/Bank.sol";
import {IReserveBridge} from "../src/interfaces/IReserveBridge.sol";

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

/// @dev ERC-20 with 18 decimals for testing decimal enforcement.
contract MockToken18 is ERC20 {
    constructor() ERC20("Token18", "TK18") {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

/// @dev Reserve bridge mock that pulls USDC from the calling Bank and can deliver inbound credits.
contract MockReserveBridge is IReserveBridge {
    IERC20 public immutable token;
    uint256 public nonce;

    event MockBridgeOut(
        address indexed sourceReserve,
        uint64 indexed destChainId,
        address indexed destReserve,
        uint256 amount,
        bytes32 messageId
    );

    constructor(IERC20 token_) {
        token = token_;
    }

    function bridgeOut(uint64 destChainId, uint256 amount, address destReserve) external returns (bytes32 messageId) {
        messageId = keccak256(abi.encode(msg.sender, destChainId, amount, destReserve, nonce++));
        token.transferFrom(msg.sender, address(this), amount);
        emit MockBridgeOut(msg.sender, destChainId, destReserve, amount, messageId);
    }

    function bridgeIn(bytes calldata message, bytes calldata /* attestation */ )
        external
        pure
        returns (bytes32 messageId)
    {
        return keccak256(message);
    }

    function bridgeType() external pure returns (bytes32) {
        return keccak256("MOCK_RESERVE_BRIDGE");
    }

    function deliver(Bank destBank, uint64 sourceChainId, uint256 amount, bytes32 messageId) external {
        MockUSDC(address(token)).mint(address(destBank), amount);
        destBank.completeReserveBridge(sourceChainId, amount, messageId);
    }
}

/// @dev Malicious token that re-enters bank.deposit during transferFrom to test reentrancy guard.
contract ReentrantToken is ERC20 {
    Bank public immutable bank;
    bool private _reentering;

    constructor(address bank_) ERC20("Reentrant", "RE") {
        bank = Bank(bank_);
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function decimals() public pure override returns (uint8) {
        return 6;
    }

    function transferFrom(address from, address to, uint256 amount) public override returns (bool) {
        if (!_reentering) {
            _reentering = true;
            // Attempt re-entry — should revert with ReentrancyGuard's error
            bank.deposit(address(this), amount);
            _reentering = false;
        }
        return super.transferFrom(from, to, amount);
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
    address public rebalancer = address(0xA8);
    address public reserveRebalancer = address(0xA9);

    // ── Contracts ──────────────────────────────────────────────────────

    SyncUSD public syncUSD;
    Bank public bank;
    MockUSDC public usdc;
    MockReserveBridge public reserveBridge;

    // ── Cached roles ───────────────────────────────────────────────────

    bytes32 public MINTER_ROLE;
    bytes32 public RELAYER_ROLE;
    bytes32 public REBALANCER_ROLE;
    bytes32 public RESERVE_REBALANCER_ROLE;
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
    event TokenAllowed(address indexed token);
    event TokenDisallowed(address indexed token);
    event RebalanceInitiated(bytes32 indexed messageId, uint64 indexed destChainId, uint256 amount);
    event RebalanceCompleted(bytes32 indexed messageId, uint64 indexed sourceChainId, uint256 amount);
    event MaxRebalanceAmountUpdated(uint256 amount);
    event AllowlistedDestChainUpdated(uint64 indexed destChainId, bool allowed);
    event AllowlistedSourceContractUpdated(uint64 indexed sourceChainId, address indexed sourceContract, bool allowed);
    event CcipRouterUpdated(address indexed router);
    event ReserveTokenUpdated(address indexed token);
    event ReserveBridgeUpdated(address indexed reserveBridge);
    event MaxReserveRebalanceAmountUpdated(uint256 amount);
    event ReserveDestinationUpdated(uint64 indexed destChainId, address indexed destReserve);
    event ReserveBridgeInitiated(
        bytes32 indexed messageId, uint64 indexed destChainId, uint256 amount, bytes32 indexed bridgeType
    );
    event ReserveBridgeCompleted(bytes32 indexed messageId, uint64 indexed sourceChainId, uint256 amount);
    event FrozenForDecommission(address indexed account);
    event PermanentlyPaused(address indexed account);

    // ── Setup ──────────────────────────────────────────────────────────

    function setUp() public {
        // Deploy SyncUSD proxy
        SyncUSD syncImpl = new SyncUSD();
        bytes memory syncInit = abi.encodeCall(SyncUSD.initialize, (admin, pauser));
        ERC1967Proxy syncProxy = new ERC1967Proxy(address(syncImpl), syncInit);
        syncUSD = SyncUSD(address(syncProxy));

        // Deploy Bank proxy
        Bank bankImpl = new Bank();
        bytes memory bankInit = abi.encodeCall(Bank.initialize, (admin, pauser, address(syncUSD), feeCollector));
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

        REBALANCER_ROLE = bank.REBALANCER_ROLE();
        vm.prank(admin);
        bank.grantRole(REBALANCER_ROLE, rebalancer);

        RESERVE_REBALANCER_ROLE = bank.RESERVE_REBALANCER_ROLE();
        vm.prank(admin);
        bank.grantRole(RESERVE_REBALANCER_ROLE, reserveRebalancer);

        PAUSER_ROLE = bank.PAUSER_ROLE();
        ADMIN_ROLE = bank.ADMIN_ROLE();

        // Deploy mock USDC and fund the user
        usdc = new MockUSDC();
        usdc.mint(user, 10_000e6);
        reserveBridge = new MockReserveBridge(IERC20(address(usdc)));

        // Allow USDC for deposit/withdrawal
        vm.prank(admin);
        bank.allowToken(address(usdc));
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

    function test_rebalancerHasRebalancerRole() public view {
        assertTrue(bank.hasRole(REBALANCER_ROLE, rebalancer));
        assertTrue(REBALANCER_ROLE != RELAYER_ROLE);
    }

    function test_reserveRebalancerHasSeparateRole() public view {
        assertTrue(bank.hasRole(RESERVE_REBALANCER_ROLE, reserveRebalancer));
        assertTrue(RESERVE_REBALANCER_ROLE != REBALANCER_ROLE);
        assertTrue(RESERVE_REBALANCER_ROLE != RELAYER_ROLE);
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
        bytes memory data = abi.encodeCall(Bank.initialize, (address(0), pauser, address(syncUSD), feeCollector));
        vm.expectRevert(Bank.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), data);
    }

    function test_initializeRevertsOnZeroPauser() public {
        Bank impl = new Bank();
        bytes memory data = abi.encodeCall(Bank.initialize, (admin, address(0), address(syncUSD), feeCollector));
        vm.expectRevert(Bank.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), data);
    }

    function test_initializeRevertsOnZeroSyncUSD() public {
        Bank impl = new Bank();
        bytes memory data = abi.encodeCall(Bank.initialize, (admin, pauser, address(0), feeCollector));
        vm.expectRevert(Bank.ZeroAddress.selector);
        new ERC1967Proxy(address(impl), data);
    }

    // ── Token whitelist ──────────────────────────────────────────────────

    function test_usdcIsAllowedAfterSetup() public view {
        assertTrue(bank.allowedTokens(address(usdc)));
        assertEq(bank.reserveToken(), address(usdc));
    }

    function test_adminCanAllowToken() public {
        MockUSDC newToken = new MockUSDC();
        vm.expectEmit(true, false, false, false);
        emit TokenAllowed(address(newToken));
        vm.prank(admin);
        bank.allowToken(address(newToken));
        assertTrue(bank.allowedTokens(address(newToken)));
    }

    function test_adminCanDisallowToken() public {
        vm.expectEmit(true, false, false, false);
        emit TokenDisallowed(address(usdc));
        vm.prank(admin);
        bank.disallowToken(address(usdc));
        assertFalse(bank.allowedTokens(address(usdc)));
    }

    function test_nonAdminCannotAllowToken() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.allowToken(address(usdc));
    }

    function test_allowTokenRevertsOnZeroAddress() public {
        vm.prank(admin);
        vm.expectRevert(Bank.ZeroAddress.selector);
        bank.allowToken(address(0));
    }

    function test_depositRevertsForDisallowedToken() public {
        MockUSDC unlisted = new MockUSDC();
        unlisted.mint(user, 100e6);

        vm.startPrank(user);
        unlisted.approve(address(bank), 100e6);
        vm.expectRevert(abi.encodeWithSelector(Bank.TokenNotAllowed.selector, address(unlisted)));
        bank.deposit(address(unlisted), 100e6);
        vm.stopPrank();
    }

    function test_withdrawRevertsForDisallowedToken() public {
        _depositForUser(100e6);

        MockUSDC unlisted = new MockUSDC();

        vm.startPrank(user);
        syncUSD.approve(address(bank), 100e6);
        vm.expectRevert(abi.encodeWithSelector(Bank.TokenNotAllowed.selector, address(unlisted)));
        bank.withdraw(address(unlisted), 100e6);
        vm.stopPrank();
    }

    function test_depositRevertsForNonSixDecimalToken() public {
        MockToken18 token18 = new MockToken18();
        vm.prank(admin);
        bank.allowToken(address(token18));

        token18.mint(user, 100e18);
        vm.startPrank(user);
        token18.approve(address(bank), 100e18);
        vm.expectRevert(abi.encodeWithSelector(Bank.InvalidTokenDecimals.selector, address(token18), uint8(18)));
        bank.deposit(address(token18), 100e18);
        vm.stopPrank();
    }

    // ── Reentrancy guard ─────────────────────────────────────────────────

    function test_depositBlocksReentrancy() public {
        ReentrantToken malicious = new ReentrantToken(address(bank));
        vm.prank(admin);
        bank.allowToken(address(malicious));

        malicious.mint(user, 200e6);
        vm.startPrank(user);
        malicious.approve(address(bank), 200e6);
        vm.expectRevert();
        bank.deposit(address(malicious), 100e6);
        vm.stopPrank();
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

    function test_adminCanGrantRebalancerRole() public {
        address newRebalancer = address(0xBC);
        vm.prank(admin);
        bank.grantRole(REBALANCER_ROLE, newRebalancer);
        assertTrue(bank.hasRole(REBALANCER_ROLE, newRebalancer));
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

    function test_rebalanceRevertsWhenPaused() public {
        _fundPool(100e6);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);

        vm.prank(pauser);
        bank.pause();

        vm.prank(rebalancer);
        vm.expectRevert();
        bank.rebalance(42, 100e6);
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

    // ── Cold path rebalance ─────────────────────────────────────────────

    function _fundPool(uint256 amount) internal {
        _giveUserSyncUSD(amount);
        vm.startPrank(user);
        syncUSD.approve(address(bank), amount);
        bank.transferHotPath(recipient, amount, 42);
        vm.stopPrank();
    }

    function test_adminCanConfigureRebalanceCapAndAllowlists() public {
        vm.expectEmit(false, false, false, true);
        emit MaxRebalanceAmountUpdated(500e6);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(500e6);
        assertEq(bank.maxRebalanceAmount(), 500e6);

        vm.expectEmit(true, false, false, true);
        emit AllowlistedDestChainUpdated(42, true);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);
        assertTrue(bank.allowlistedDestChains(42));

        vm.expectEmit(true, true, false, true);
        emit AllowlistedSourceContractUpdated(99, address(bank), true);
        vm.prank(admin);
        bank.setAllowlistedSourceContract(99, address(bank), true);
        assertTrue(bank.allowlistedSourceContracts(99, address(bank)));

        vm.expectEmit(true, false, false, false);
        emit CcipRouterUpdated(address(this));
        vm.prank(admin);
        bank.setCcipRouter(address(this));
        assertEq(bank.ccipRouter(), address(this));
    }

    function test_nonAdminCannotConfigureRebalance() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setMaxRebalanceAmount(500e6);

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setAllowlistedDestChain(42, true);

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setAllowlistedSourceContract(99, address(bank), true);

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setCcipRouter(address(this));
    }

    function test_rebalanceBurnsPoolAndEmitsMessageId() public {
        uint256 amount = 100e6;
        _fundPool(amount);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(amount);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);

        bytes32 expectedMessageId = keccak256(abi.encode(block.chainid, address(bank), uint64(42), amount, uint256(0)));
        vm.expectEmit(true, true, false, true);
        emit RebalanceInitiated(expectedMessageId, 42, amount);

        vm.prank(rebalancer);
        bytes32 messageId = bank.rebalance(42, amount);

        assertEq(messageId, expectedMessageId);
        assertEq(syncUSD.balanceOf(address(bank)), 0);
        assertEq(bank.poolDepth(), 0);
    }

    function test_rebalanceRevertsForNonRebalancer() public {
        _fundPool(100e6);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.rebalance(42, 100e6);
    }

    function test_rebalanceRevertsWhenAmountExceedsCap() public {
        _fundPool(200e6);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);

        vm.prank(rebalancer);
        vm.expectRevert(abi.encodeWithSelector(Bank.RebalanceCapExceeded.selector, 200e6, 100e6));
        bank.rebalance(42, 200e6);
        assertEq(bank.poolDepth(), 200e6);
    }

    function test_rebalanceRevertsWhenDestNotAllowlisted() public {
        _fundPool(100e6);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(100e6);

        vm.prank(rebalancer);
        vm.expectRevert(abi.encodeWithSelector(Bank.DestChainNotAllowlisted.selector, uint64(42)));
        bank.rebalance(42, 100e6);
    }

    function test_rebalanceRevertsWhenPoolDepthInsufficient() public {
        _fundPool(50e6);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);

        vm.prank(rebalancer);
        vm.expectRevert(Bank.InsufficientPoolLiquidity.selector);
        bank.rebalance(42, 100e6);
    }

    function test_ccipReceiveMintsPoolAndRejectsReplay() public {
        bytes32 messageId = keccak256("ccip-message");
        uint256 amount = 250e6;
        vm.prank(admin);
        bank.setAllowlistedSourceContract(99, address(bank), true);
        vm.prank(admin);
        bank.setCcipRouter(address(this));

        vm.expectEmit(true, true, false, true);
        emit RebalanceCompleted(messageId, 99, amount);
        bank.ccipReceive(99, address(bank), amount, messageId);

        assertEq(syncUSD.balanceOf(address(bank)), amount);
        assertEq(bank.poolDepth(), amount);
        assertTrue(bank.processedMessages(messageId));

        vm.expectRevert(abi.encodeWithSelector(Bank.RebalanceMessageAlreadyProcessed.selector, messageId));
        bank.ccipReceive(99, address(bank), amount, messageId);
    }

    function test_ccipReceiveRejectsNonAllowlistedSource() public {
        bytes32 messageId = keccak256("ccip-message");
        vm.prank(admin);
        bank.setCcipRouter(address(this));

        vm.expectRevert(abi.encodeWithSelector(Bank.SourceContractNotAllowlisted.selector, uint64(99), address(bank)));
        bank.ccipReceive(99, address(bank), 100e6, messageId);
    }

    function test_ccipReceiveRejectsUnauthorizedRouter() public {
        bytes32 messageId = keccak256("ccip-message");
        vm.prank(admin);
        bank.setCcipRouter(address(this));
        vm.prank(admin);
        bank.setAllowlistedSourceContract(99, address(bank), true);

        vm.prank(unauthorized);
        vm.expectRevert(abi.encodeWithSelector(Bank.UnauthorizedCcipRouter.selector, unauthorized));
        bank.ccipReceive(99, address(bank), 100e6, messageId);
    }

    // ── USDC reserve rebalance ─────────────────────────────────────────

    function _depositReserve(uint256 amount) internal {
        vm.startPrank(user);
        usdc.approve(address(bank), amount);
        bank.deposit(address(usdc), amount);
        vm.stopPrank();
    }

    function _configureReserveBridge(uint256 cap) internal {
        vm.prank(admin);
        bank.setMaxReserveRebalanceAmount(cap);
        vm.prank(admin);
        bank.setReserveBridge(reserveBridge);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);
        vm.prank(admin);
        bank.setReserveDestination(42, address(bank));
    }

    function test_adminCanConfigureReserveBridge() public {
        vm.expectEmit(false, false, false, true);
        emit MaxReserveRebalanceAmountUpdated(500e6);
        vm.prank(admin);
        bank.setMaxReserveRebalanceAmount(500e6);
        assertEq(bank.maxReserveRebalanceAmount(), 500e6);

        vm.expectEmit(true, false, false, true);
        emit ReserveBridgeUpdated(address(reserveBridge));
        vm.prank(admin);
        bank.setReserveBridge(reserveBridge);
        assertEq(address(bank.reserveBridge()), address(reserveBridge));

        vm.expectEmit(true, true, false, true);
        emit ReserveDestinationUpdated(42, address(bank));
        vm.prank(admin);
        bank.setReserveDestination(42, address(bank));
        assertEq(bank.reserveDestinations(42), address(bank));

        MockUSDC newToken = new MockUSDC();
        vm.prank(admin);
        bank.allowToken(address(newToken));

        vm.expectEmit(true, false, false, true);
        emit ReserveTokenUpdated(address(newToken));
        vm.prank(admin);
        bank.setReserveToken(address(newToken));
        assertEq(bank.reserveToken(), address(newToken));
    }

    function test_nonAdminCannotConfigureReserveBridge() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setMaxReserveRebalanceAmount(500e6);

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setReserveBridge(reserveBridge);

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setReserveDestination(42, address(bank));

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.setReserveToken(address(usdc));
    }

    function test_reserveDepthReflectsReserveTokenBalance() public {
        assertEq(bank.reserveDepth(), 0);

        _depositReserve(250e6);

        assertEq(usdc.balanceOf(address(bank)), 250e6);
        assertEq(bank.reserveDepth(), 250e6);
    }

    function test_bridgeReserveTransfersReserveAndEmitsMessageId() public {
        uint256 amount = 100e6;
        _depositReserve(250e6);
        _configureReserveBridge(amount);

        bytes32 expectedMessageId = keccak256(abi.encode(address(bank), uint64(42), amount, address(bank), uint256(0)));
        bytes32 expectedBridgeType = keccak256("MOCK_RESERVE_BRIDGE");

        vm.expectEmit(true, true, true, true);
        emit ReserveBridgeInitiated(expectedMessageId, 42, amount, expectedBridgeType);

        vm.prank(reserveRebalancer);
        bytes32 messageId = bank.bridgeReserve(42, amount);

        assertEq(messageId, expectedMessageId);
        assertEq(usdc.balanceOf(address(reserveBridge)), amount);
        assertEq(bank.reserveDepth(), 150e6);
        assertEq(usdc.allowance(address(bank), address(reserveBridge)), 0);
    }

    function test_completeReserveBridgeRecordsInboundDeliveryAndRejectsReplay() public {
        uint256 amount = 100e6;
        _depositReserve(250e6);
        _configureReserveBridge(amount);

        vm.prank(reserveRebalancer);
        bytes32 messageId = bank.bridgeReserve(42, amount);

        vm.expectEmit(true, true, false, true);
        emit ReserveBridgeCompleted(messageId, 99, amount);
        reserveBridge.deliver(bank, 99, amount, messageId);

        assertEq(bank.reserveDepth(), 250e6);
        assertTrue(bank.processedReserveMessages(messageId));

        vm.expectRevert(abi.encodeWithSelector(Bank.ReserveBridgeMessageAlreadyProcessed.selector, messageId));
        reserveBridge.deliver(bank, 99, amount, messageId);
    }

    function test_completeReserveBridgeRejectsUnauthorizedCaller() public {
        _configureReserveBridge(100e6);
        bytes32 messageId = keccak256("reserve-message");

        vm.prank(unauthorized);
        vm.expectRevert(abi.encodeWithSelector(Bank.UnauthorizedReserveBridge.selector, unauthorized));
        bank.completeReserveBridge(99, 100e6, messageId);
    }

    function test_bridgeReserveRevertsForNonReserveRebalancer() public {
        _depositReserve(100e6);
        _configureReserveBridge(100e6);

        vm.prank(unauthorized);
        vm.expectRevert();
        bank.bridgeReserve(42, 100e6);
    }

    function test_bridgeReserveRevertsWhenAmountExceedsCap() public {
        _depositReserve(200e6);
        _configureReserveBridge(100e6);

        vm.prank(reserveRebalancer);
        vm.expectRevert(abi.encodeWithSelector(Bank.ReserveRebalanceCapExceeded.selector, 200e6, 100e6));
        bank.bridgeReserve(42, 200e6);
        assertEq(bank.reserveDepth(), 200e6);
    }

    function test_bridgeReserveRevertsWhenDestNotAllowlisted() public {
        _depositReserve(100e6);
        vm.prank(admin);
        bank.setMaxReserveRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setReserveBridge(reserveBridge);
        vm.prank(admin);
        bank.setReserveDestination(42, address(bank));

        vm.prank(reserveRebalancer);
        vm.expectRevert(abi.encodeWithSelector(Bank.DestChainNotAllowlisted.selector, uint64(42)));
        bank.bridgeReserve(42, 100e6);
    }

    function test_bridgeReserveRevertsWhenDestinationReserveNotConfigured() public {
        _depositReserve(100e6);
        vm.prank(admin);
        bank.setMaxReserveRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setReserveBridge(reserveBridge);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);

        vm.prank(reserveRebalancer);
        vm.expectRevert(abi.encodeWithSelector(Bank.DestChainNotAllowlisted.selector, uint64(42)));
        bank.bridgeReserve(42, 100e6);
    }

    function test_bridgeReserveRevertsWhenReserveDepthInsufficient() public {
        _depositReserve(50e6);
        _configureReserveBridge(100e6);

        vm.prank(reserveRebalancer);
        vm.expectRevert(Bank.InsufficientReserveLiquidity.selector);
        bank.bridgeReserve(42, 100e6);
    }

    function test_bridgeReserveRevertsWhenAdapterNotSet() public {
        _depositReserve(100e6);
        vm.prank(admin);
        bank.setMaxReserveRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);
        vm.prank(admin);
        bank.setReserveDestination(42, address(bank));

        vm.prank(reserveRebalancer);
        vm.expectRevert(Bank.ReserveBridgeNotSet.selector);
        bank.bridgeReserve(42, 100e6);
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
        assertTrue(bank.allowedTokens(address(usdc)));
    }

    function test_upgradeRevertsForUnauthorized() public {
        BankV2 newImpl = new BankV2();
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.upgradeToAndCall(address(newImpl), "");
    }

    // ── Decommission freeze ────────────────────────────────────────────

    function test_adminCanFreezeForDecommissionOnce() public {
        vm.expectEmit(true, false, false, false);
        emit FrozenForDecommission(admin);

        vm.prank(admin);
        bank.freezeForDecommission();
        assertTrue(bank.frozen());

        vm.prank(admin);
        vm.expectRevert(Bank.ContractFrozenForDecommission.selector);
        bank.freezeForDecommission();
    }

    function test_nonAdminCannotFreezeForDecommission() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.freezeForDecommission();
    }

    function test_frozenDepositReverts() public {
        vm.prank(admin);
        bank.freezeForDecommission();

        vm.startPrank(user);
        usdc.approve(address(bank), 100e6);
        vm.expectRevert(Bank.ContractFrozenForDecommission.selector);
        bank.deposit(address(usdc), 100e6);
        vm.stopPrank();
    }

    function test_frozenTransferHotPathReverts() public {
        _giveUserSyncUSD(100e6);

        vm.prank(admin);
        bank.freezeForDecommission();

        vm.startPrank(user);
        syncUSD.approve(address(bank), 100e6);
        vm.expectRevert(Bank.ContractFrozenForDecommission.selector);
        bank.transferHotPath(recipient, 100e6, 42);
        vm.stopPrank();
    }

    function test_frozenReleaseHotPathReverts() public {
        _fundPool(100e6);

        vm.prank(admin);
        bank.freezeForDecommission();

        vm.prank(relayer);
        vm.expectRevert(Bank.ContractFrozenForDecommission.selector);
        bank.releaseHotPath(recipient, 100e6, bytes32(uint256(1)));
    }

    function test_frozenWithdrawStillAllowed() public {
        _depositForUser(100e6);

        vm.prank(admin);
        bank.freezeForDecommission();

        vm.startPrank(user);
        syncUSD.approve(address(bank), 100e6);
        bank.withdraw(address(usdc), 100e6);
        vm.stopPrank();

        assertEq(syncUSD.balanceOf(user), 0);
        assertEq(usdc.balanceOf(user), 10_000e6);
    }

    function test_frozenRebalanceStillAllowed() public {
        _fundPool(100e6);
        vm.prank(admin);
        bank.setMaxRebalanceAmount(100e6);
        vm.prank(admin);
        bank.setAllowlistedDestChain(42, true);
        vm.prank(admin);
        bank.freezeForDecommission();

        vm.prank(rebalancer);
        bytes32 messageId = bank.rebalance(42, 100e6);

        assertTrue(messageId != bytes32(0));
        assertEq(bank.poolDepth(), 0);
    }

    function test_frozenBridgeReserveStillAllowed() public {
        _depositReserve(100e6);
        _configureReserveBridge(100e6);
        vm.prank(admin);
        bank.freezeForDecommission();

        vm.prank(reserveRebalancer);
        bytes32 messageId = bank.bridgeReserve(42, 100e6);

        assertTrue(messageId != bytes32(0));
        assertEq(bank.reserveDepth(), 0);
    }

    function test_adminCanPausePermanentlyAndCannotUnpause() public {
        vm.expectEmit(true, false, false, false);
        emit PermanentlyPaused(admin);

        vm.prank(admin);
        bank.pausePermanently();
        assertTrue(bank.paused());
        assertTrue(bank.permanentPause());

        vm.prank(pauser);
        vm.expectRevert(Bank.ContractPermanentlyPaused.selector);
        bank.unpause();
    }

    function test_nonAdminCannotPausePermanently() public {
        vm.prank(unauthorized);
        vm.expectRevert();
        bank.pausePermanently();
    }
}
