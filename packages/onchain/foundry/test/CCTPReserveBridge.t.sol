// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IAccessControl} from "@openzeppelin/contracts/access/IAccessControl.sol";
import {
    CCTPReserveBridge,
    ITokenMessenger,
    IMessageTransmitter,
    IBankReserveSink
} from "../src/CCTPReserveBridge.sol";

// ── Mocks ──────────────────────────────────────────────────────────────

contract MockUSDC is ERC20 {
    constructor() ERC20("USD Coin", "USDC") {}

    function decimals() public pure override returns (uint8) {
        return 6;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    function burn(address from, uint256 amount) external {
        _burn(from, amount);
    }
}

/// @dev Records the last call and burns the pulled USDC, mirroring CCTP TokenMessenger semantics.
contract MockTokenMessenger is ITokenMessenger {
    MockUSDC public immutable usdc;
    uint64 public nextNonce = 1;

    struct LastCall {
        uint256 amount;
        uint32 destinationDomain;
        bytes32 mintRecipient;
        address burnToken;
        bytes32 destinationCaller;
        address caller;
    }

    LastCall public last;

    constructor(MockUSDC usdc_) {
        usdc = usdc_;
    }

    function depositForBurnWithCaller(
        uint256 amount,
        uint32 destinationDomain,
        bytes32 mintRecipient,
        address burnToken,
        bytes32 destinationCaller
    ) external returns (uint64 nonce) {
        last = LastCall(amount, destinationDomain, mintRecipient, burnToken, destinationCaller, msg.sender);
        usdc.transferFrom(msg.sender, address(this), amount);
        usdc.burn(address(this), amount);
        nonce = nextNonce++;
    }
}

/// @dev Verifies the attestation is non-empty (stand-in for signature verification) and mints USDC to
///      the body's `mintRecipient`.
contract MockMessageTransmitter is IMessageTransmitter {
    MockUSDC public immutable usdc;
    bool public shouldFail;

    constructor(MockUSDC usdc_) {
        usdc = usdc_;
    }

    function setShouldFail(bool v) external {
        shouldFail = v;
    }

    function receiveMessage(bytes calldata message, bytes calldata attestation) external returns (bool) {
        if (shouldFail) return false;
        require(attestation.length > 0, "empty attestation");
        // Body layout starts at offset 116. mintRecipient is bytes 4..36 of body, amount is bytes 36..68.
        bytes32 mintRecipient;
        uint256 amount;
        assembly {
            mintRecipient := calldataload(add(message.offset, 152))
            amount := calldataload(add(message.offset, 184))
        }
        usdc.mint(address(uint160(uint256(mintRecipient))), amount);
        return true;
    }
}

/// @dev Records the inbound completion and rejects duplicate messageIds (mirrors Bank semantics).
contract MockBank is IBankReserveSink {
    struct Completion {
        uint64 sourceChainId;
        uint256 amount;
        bytes32 messageId;
    }

    Completion[] public completions;
    mapping(bytes32 => bool) public processed;
    address public expectedCaller;

    error DuplicateMessage(bytes32 messageId);
    error UnexpectedCaller(address caller);

    function setExpectedCaller(address c) external {
        expectedCaller = c;
    }

    function completeReserveBridge(uint64 sourceChainId, uint256 amount, bytes32 messageId) external {
        if (expectedCaller != address(0) && msg.sender != expectedCaller) revert UnexpectedCaller(msg.sender);
        if (processed[messageId]) revert DuplicateMessage(messageId);
        processed[messageId] = true;
        completions.push(Completion(sourceChainId, amount, messageId));
    }

    function completionsLength() external view returns (uint256) {
        return completions.length;
    }
}

// ── Test ───────────────────────────────────────────────────────────────

contract CCTPReserveBridgeTest is Test {
    // Actors
    address admin = address(0xA1);
    address bankActor = address(0xB1);
    address other = address(0xCAFE);

    // Chain / domain mapping under test
    uint64 constant LOCAL_CHAIN_ID = 8453; // Base
    uint64 constant REMOTE_CHAIN_ID = 42161; // Arbitrum
    uint32 constant LOCAL_DOMAIN = 6;
    uint32 constant REMOTE_DOMAIN = 3;

    MockUSDC usdc;
    MockTokenMessenger tokenMessenger;
    MockMessageTransmitter messageTransmitter;
    MockBank mockBank;
    CCTPReserveBridge adapter;
    address remoteAdapterAddr = address(0xAFAFAF);

    function setUp() public {
        usdc = new MockUSDC();
        tokenMessenger = new MockTokenMessenger(usdc);
        messageTransmitter = new MockMessageTransmitter(usdc);
        mockBank = new MockBank();

        vm.prank(admin);
        adapter = new CCTPReserveBridge(
            IERC20(address(usdc)),
            ITokenMessenger(address(tokenMessenger)),
            IMessageTransmitter(address(messageTransmitter)),
            LOCAL_DOMAIN,
            admin
        );

        vm.startPrank(admin);
        adapter.setBank(address(mockBank));
        adapter.setChainDomain(LOCAL_CHAIN_ID, LOCAL_DOMAIN);
        adapter.setChainDomain(REMOTE_CHAIN_ID, REMOTE_DOMAIN);
        adapter.setRemoteAdapter(REMOTE_DOMAIN, remoteAdapterAddr);
        vm.stopPrank();

        mockBank.setExpectedCaller(address(adapter));
    }

    // ── Construction & governance ───────────────────────────────────────

    function test_constructorRevertsOnZeroUsdc() public {
        vm.expectRevert(CCTPReserveBridge.ZeroAddress.selector);
        new CCTPReserveBridge(
            IERC20(address(0)),
            ITokenMessenger(address(tokenMessenger)),
            IMessageTransmitter(address(messageTransmitter)),
            LOCAL_DOMAIN,
            admin
        );
    }

    function test_constructorRevertsOnZeroAdmin() public {
        vm.expectRevert(CCTPReserveBridge.ZeroAddress.selector);
        new CCTPReserveBridge(
            IERC20(address(usdc)),
            ITokenMessenger(address(tokenMessenger)),
            IMessageTransmitter(address(messageTransmitter)),
            LOCAL_DOMAIN,
            address(0)
        );
    }

    function test_setBankRevertsForNonAdmin() public {
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, other, bytes32(0))
        );
        vm.prank(other);
        adapter.setBank(bankActor);
    }

    function test_setBankRevertsOnZero() public {
        vm.expectRevert(CCTPReserveBridge.ZeroAddress.selector);
        vm.prank(admin);
        adapter.setBank(address(0));
    }

    function test_setChainDomainWritesBothDirections() public {
        vm.prank(admin);
        adapter.setChainDomain(10, 2);
        assertEq(adapter.chainIdToDomain(10), 2);
        assertEq(adapter.domainToChainId(2), 10);
        assertTrue(adapter.chainConfigured(10));
        assertTrue(adapter.domainConfigured(2));
    }

    function test_setRemoteAdapterRevertsOnZero() public {
        vm.expectRevert(CCTPReserveBridge.ZeroAddress.selector);
        vm.prank(admin);
        adapter.setRemoteAdapter(REMOTE_DOMAIN, address(0));
    }

    function test_bridgeTypeIsCCTP() public view {
        assertEq(adapter.bridgeType(), bytes32("CCTP"));
    }

    // ── bridgeOut ───────────────────────────────────────────────────────

    function _fundAndApproveBank(uint256 amount) internal {
        usdc.mint(address(mockBank), amount);
        vm.prank(address(mockBank));
        usdc.approve(address(adapter), amount);
    }

    function test_bridgeOutHappyPath() public {
        uint256 amount = 100_000e6;
        address destReserve = address(0xDE57);
        _fundAndApproveBank(amount);

        vm.prank(address(mockBank));
        bytes32 messageId = adapter.bridgeOut(REMOTE_CHAIN_ID, amount, destReserve);

        // Bank's USDC was pulled and burned by the (mock) TokenMessenger.
        assertEq(usdc.balanceOf(address(mockBank)), 0);
        assertEq(usdc.balanceOf(address(adapter)), 0);
        assertEq(usdc.balanceOf(address(tokenMessenger)), 0);

        // TokenMessenger received the right args.
        (
            uint256 amt,
            uint32 destDom,
            bytes32 mintRecipient,
            address burnToken,
            bytes32 destCaller,
            address caller
        ) = tokenMessenger.last();
        assertEq(amt, amount);
        assertEq(destDom, REMOTE_DOMAIN);
        assertEq(mintRecipient, bytes32(uint256(uint160(destReserve))));
        assertEq(burnToken, address(usdc));
        assertEq(destCaller, bytes32(uint256(uint160(remoteAdapterAddr))));
        assertEq(caller, address(adapter));

        // messageId formula is deterministic: keccak256(BRIDGE_TYPE, localDomain, nonce).
        bytes32 expected = keccak256(abi.encode(bytes32("CCTP"), LOCAL_DOMAIN, uint64(1)));
        assertEq(messageId, expected);
    }

    function test_bridgeOutRevertsForNonBank() public {
        _fundAndApproveBank(1e6);
        vm.expectRevert(abi.encodeWithSelector(CCTPReserveBridge.OnlyBank.selector, other));
        vm.prank(other);
        adapter.bridgeOut(REMOTE_CHAIN_ID, 1e6, address(0xDE57));
    }

    function test_bridgeOutRevertsWhenBankUnset() public {
        // Deploy a fresh adapter without setBank.
        CCTPReserveBridge fresh = new CCTPReserveBridge(
            IERC20(address(usdc)),
            ITokenMessenger(address(tokenMessenger)),
            IMessageTransmitter(address(messageTransmitter)),
            LOCAL_DOMAIN,
            admin
        );
        vm.expectRevert(CCTPReserveBridge.BankNotSet.selector);
        fresh.bridgeOut(REMOTE_CHAIN_ID, 1e6, address(0xDE57));
    }

    function test_bridgeOutRevertsOnZeroAmount() public {
        vm.expectRevert(CCTPReserveBridge.ZeroAmount.selector);
        vm.prank(address(mockBank));
        adapter.bridgeOut(REMOTE_CHAIN_ID, 0, address(0xDE57));
    }

    function test_bridgeOutRevertsOnZeroDestReserve() public {
        vm.expectRevert(CCTPReserveBridge.ZeroAddress.selector);
        vm.prank(address(mockBank));
        adapter.bridgeOut(REMOTE_CHAIN_ID, 1e6, address(0));
    }

    function test_bridgeOutRevertsOnUnconfiguredChain() public {
        vm.expectRevert(abi.encodeWithSelector(CCTPReserveBridge.ChainNotConfigured.selector, uint64(999)));
        vm.prank(address(mockBank));
        adapter.bridgeOut(999, 1e6, address(0xDE57));
    }

    function test_bridgeOutRevertsWhenRemoteAdapterMissing() public {
        // Configure a new chain but skip setRemoteAdapter.
        vm.prank(admin);
        adapter.setChainDomain(10, 2);
        vm.expectRevert(abi.encodeWithSelector(CCTPReserveBridge.RemoteAdapterNotConfigured.selector, uint32(2)));
        vm.prank(address(mockBank));
        adapter.bridgeOut(10, 1e6, address(0xDE57));
    }

    // ── bridgeIn ────────────────────────────────────────────────────────

    /// @dev Build a syntactically valid CCTP v1 burn message: 116-byte header + 132-byte body.
    function _buildMessage(uint32 sourceDomain, uint64 nonce, address mintRecipient, uint256 amount, address sender)
        internal
        view
        returns (bytes memory)
    {
        return abi.encodePacked(
            // header
            uint32(0), // version
            sourceDomain,
            LOCAL_DOMAIN, // destDomain
            nonce,
            bytes32(uint256(uint160(address(tokenMessenger)))), // header.sender — irrelevant for our auth
            bytes32(uint256(uint160(address(messageTransmitter)))), // header.recipient
            bytes32(uint256(uint160(address(adapter)))), // header.destCaller
            // body
            uint32(0), // bodyVersion
            bytes32(uint256(uint160(address(usdc)))), // burnToken
            bytes32(uint256(uint160(mintRecipient))),
            amount,
            bytes32(uint256(uint160(sender))) // body.messageSender — paired remote adapter
        );
    }

    function test_bridgeInHappyPath() public {
        uint256 amount = 250_000e6;
        uint64 nonce = 7;
        bytes memory message = _buildMessage(REMOTE_DOMAIN, nonce, address(mockBank), amount, remoteAdapterAddr);
        bytes memory attestation = hex"deadbeef";

        bytes32 messageId = adapter.bridgeIn(message, attestation);

        // USDC was minted to Bank.
        assertEq(usdc.balanceOf(address(mockBank)), amount);

        // Bank recorded the completion with the right values.
        assertEq(mockBank.completionsLength(), 1);
        (uint64 src, uint256 amt, bytes32 mid) = mockBank.completions(0);
        assertEq(src, REMOTE_CHAIN_ID);
        assertEq(amt, amount);
        assertEq(mid, messageId);

        // Same messageId formula as the source side.
        assertEq(messageId, keccak256(abi.encode(bytes32("CCTP"), REMOTE_DOMAIN, nonce)));
    }

    function test_bridgeInRevertsWhenBankUnset() public {
        CCTPReserveBridge fresh = new CCTPReserveBridge(
            IERC20(address(usdc)),
            ITokenMessenger(address(tokenMessenger)),
            IMessageTransmitter(address(messageTransmitter)),
            LOCAL_DOMAIN,
            admin
        );
        bytes memory msgBytes = _buildMessage(REMOTE_DOMAIN, 1, address(mockBank), 1e6, remoteAdapterAddr);
        vm.expectRevert(CCTPReserveBridge.BankNotSet.selector);
        fresh.bridgeIn(msgBytes, hex"01");
    }

    function test_bridgeInRevertsOnShortMessage() public {
        bytes memory tooShort = new bytes(50);
        vm.expectRevert(abi.encodeWithSelector(CCTPReserveBridge.MessageTooShort.selector, uint256(50)));
        adapter.bridgeIn(tooShort, hex"01");
    }

    function test_bridgeInRevertsWhenSourceDomainNotConfigured() public {
        bytes memory message = _buildMessage(uint32(99), 1, address(mockBank), 1e6, remoteAdapterAddr);
        vm.expectRevert(abi.encodeWithSelector(CCTPReserveBridge.DomainNotConfigured.selector, uint32(99)));
        adapter.bridgeIn(message, hex"01");
    }

    function test_bridgeInRevertsWhenRemoteAdapterUnset() public {
        // Configure domain mapping but no remote adapter.
        vm.prank(admin);
        adapter.setChainDomain(uint64(999), uint32(99));
        bytes memory message = _buildMessage(uint32(99), 1, address(mockBank), 1e6, remoteAdapterAddr);
        vm.expectRevert(abi.encodeWithSelector(CCTPReserveBridge.RemoteAdapterNotConfigured.selector, uint32(99)));
        adapter.bridgeIn(message, hex"01");
    }

    function test_bridgeInRevertsOnUnexpectedSender() public {
        address impostor = address(0xBAD);
        bytes memory message = _buildMessage(REMOTE_DOMAIN, 1, address(mockBank), 1e6, impostor);
        bytes32 expected = bytes32(uint256(uint160(remoteAdapterAddr)));
        bytes32 actual = bytes32(uint256(uint160(impostor)));
        vm.expectRevert(abi.encodeWithSelector(CCTPReserveBridge.UnexpectedSender.selector, expected, actual));
        adapter.bridgeIn(message, hex"01");
    }

    function test_bridgeInRevertsWhenReceiveMessageFails() public {
        messageTransmitter.setShouldFail(true);
        bytes memory message = _buildMessage(REMOTE_DOMAIN, 1, address(mockBank), 1e6, remoteAdapterAddr);
        vm.expectRevert(CCTPReserveBridge.ReceiveMessageFailed.selector);
        adapter.bridgeIn(message, hex"01");
    }

    function test_bridgeInIdempotencyEnforcedByBank() public {
        bytes memory message = _buildMessage(REMOTE_DOMAIN, 1, address(mockBank), 1e6, remoteAdapterAddr);
        adapter.bridgeIn(message, hex"01");
        bytes32 messageId = keccak256(abi.encode(bytes32("CCTP"), REMOTE_DOMAIN, uint64(1)));
        vm.expectRevert(abi.encodeWithSelector(MockBank.DuplicateMessage.selector, messageId));
        adapter.bridgeIn(message, hex"01");
    }

    // ── End-to-end symmetry ─────────────────────────────────────────────

    function test_messageIdSymmetricAcrossSourceAndDest() public {
        uint256 amount = 50_000e6;
        address destReserve = address(0xDE57);
        _fundAndApproveBank(amount);

        vm.prank(address(mockBank));
        bytes32 outboundId = adapter.bridgeOut(REMOTE_CHAIN_ID, amount, destReserve);

        // Build the inbound message that would arrive on the remote chain, using the same nonce the
        // mock TokenMessenger handed back (1). The remote chain would compute its own messageId from
        // (sourceDomain=LOCAL_DOMAIN, nonce=1) — identical to outboundId.
        bytes32 inboundId = keccak256(abi.encode(bytes32("CCTP"), LOCAL_DOMAIN, uint64(1)));
        assertEq(outboundId, inboundId);
    }
}
