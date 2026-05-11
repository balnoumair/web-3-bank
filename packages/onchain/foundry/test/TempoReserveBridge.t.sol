// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IAccessControl} from "@openzeppelin/contracts/access/IAccessControl.sol";

import {
    TempoReserveBridge,
    ILayerZeroEndpointV2,
    MessagingFee,
    MessagingParams,
    MessagingReceipt,
    Origin,
    IBankReserveSink
} from "../src/TempoReserveBridge.sol";

// ── Mocks ──────────────────────────────────────────────────────────────

contract MockUSDC is ERC20 {
    constructor() ERC20("USD Coin", "USDC") {}

    function decimals() public pure override returns (uint8) {
        return 6;
    }

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract MockLzEndpoint is ILayerZeroEndpointV2 {
    uint256 public nativeFee;
    uint64 public nextNonce = 1;

    struct LastSend {
        uint32 dstEid;
        bytes32 receiver;
        bytes message;
        uint256 value;
        address sender;
    }

    LastSend public lastSend;

    constructor(uint256 nativeFee_) {
        nativeFee = nativeFee_;
    }

    function setNativeFee(uint256 v) external {
        nativeFee = v;
    }

    function quote(MessagingParams calldata, address) external view returns (MessagingFee memory) {
        return MessagingFee({nativeFee: nativeFee, lzTokenFee: 0});
    }

    function send(MessagingParams calldata params, address /* refund */)
        external
        payable
        returns (MessagingReceipt memory)
    {
        require(msg.value >= nativeFee, "fee underpaid");
        lastSend = LastSend(params.dstEid, params.receiver, params.message, msg.value, msg.sender);
        return MessagingReceipt({
            guid: keccak256(abi.encode(params, nextNonce)),
            nonce: nextNonce++,
            fee: MessagingFee({nativeFee: msg.value, lzTokenFee: 0})
        });
    }
}

contract MockBank is IBankReserveSink {
    struct Completion {
        uint64 sourceChainId;
        uint256 amount;
        bytes32 messageId;
    }

    Completion[] public completions;
    mapping(bytes32 => bool) public processed;

    error DuplicateMessage(bytes32 messageId);

    function completeReserveBridge(uint64 sourceChainId, uint256 amount, bytes32 messageId) external {
        if (processed[messageId]) revert DuplicateMessage(messageId);
        processed[messageId] = true;
        completions.push(Completion(sourceChainId, amount, messageId));
    }

    function completionsLength() external view returns (uint256) {
        return completions.length;
    }
}

// ── Test ───────────────────────────────────────────────────────────────

contract TempoReserveBridgeTest is Test {
    // Actors
    address admin = address(0xA1);
    address other = address(0xCAFE);

    // Signer keypairs (deterministic via Foundry).
    uint256 sk1 = 0xA11CE;
    uint256 sk2 = 0xB0B;
    uint256 sk3 = 0xC413;
    address signer1;
    address signer2;
    address signer3;

    // Chains / EIDs
    uint64 constant LOCAL_CHAIN_ID = 84532; // Base Sepolia
    uint64 constant TEMPO_CHAIN_ID = 70_000; // hypothetical Tempo Moderato chain id
    uint32 constant LOCAL_EID = 40_245;
    uint32 constant TEMPO_EID = 40_999;

    MockUSDC usdc;
    MockLzEndpoint lzEndpoint;
    MockBank bank;
    TempoReserveBridge bridge;
    bytes32 remoteAdapterBytes32 = bytes32(uint256(uint160(0xBEEF)));

    function setUp() public {
        signer1 = vm.addr(sk1);
        signer2 = vm.addr(sk2);
        signer3 = vm.addr(sk3);

        usdc = new MockUSDC();
        lzEndpoint = new MockLzEndpoint(0.001 ether);
        bank = new MockBank();

        vm.prank(admin);
        bridge = new TempoReserveBridge(IERC20(address(usdc)), ILayerZeroEndpointV2(address(lzEndpoint)), LOCAL_EID, admin);

        vm.startPrank(admin);
        bridge.setBank(address(bank));
        bridge.setChainEid(LOCAL_CHAIN_ID, LOCAL_EID);
        bridge.setChainEid(TEMPO_CHAIN_ID, TEMPO_EID);
        bridge.setRemoteAdapter(TEMPO_EID, remoteAdapterBytes32);
        bridge.setRemoteAdapter(LOCAL_EID, bytes32(uint256(uint160(address(bridge)))));
        address[] memory signers = new address[](3);
        signers[0] = _orderedFirst(signer1, signer2, signer3);
        signers[2] = _orderedThird(signer1, signer2, signer3);
        signers[1] = _orderedSecond(signer1, signer2, signer3);
        bridge.setSigners(signers, 2);
        vm.stopPrank();

        // Pre-fund the adapter with ETH for LZ fees.
        vm.deal(address(bridge), 1 ether);
    }

    // Helpers to assemble the signer list in ascending address order for deterministic tests.
    function _orderedFirst(address a, address b, address c) internal pure returns (address) {
        address lo = a < b ? a : b;
        return lo < c ? lo : c;
    }

    function _orderedThird(address a, address b, address c) internal pure returns (address) {
        address hi = a > b ? a : b;
        return hi > c ? hi : c;
    }

    function _orderedSecond(address a, address b, address c) internal pure returns (address) {
        address lo = _orderedFirst(a, b, c);
        address hi = _orderedThird(a, b, c);
        if (a != lo && a != hi) return a;
        if (b != lo && b != hi) return b;
        return c;
    }

    // ── Construction & governance ──────────────────────────────────────

    function test_constructorRevertsOnZeroUsdc() public {
        vm.expectRevert(TempoReserveBridge.ZeroAddress.selector);
        new TempoReserveBridge(IERC20(address(0)), ILayerZeroEndpointV2(address(lzEndpoint)), LOCAL_EID, admin);
    }

    function test_setBankRevertsForNonAdmin() public {
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, other, bytes32(0))
        );
        vm.prank(other);
        bridge.setBank(address(bank));
    }

    function test_setSignersRejectsDuplicates() public {
        address[] memory dup = new address[](2);
        dup[0] = address(0x10);
        dup[1] = address(0x10);
        TempoReserveBridge fresh = new TempoReserveBridge(
            IERC20(address(usdc)), ILayerZeroEndpointV2(address(lzEndpoint)), LOCAL_EID, admin
        );
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.DuplicateSigner.selector, address(0x10)));
        fresh.setSigners(dup, 1);
    }

    function test_setSignersRejectsZeroThreshold() public {
        address[] memory s = new address[](2);
        s[0] = address(0x10);
        s[1] = address(0x20);
        TempoReserveBridge fresh = new TempoReserveBridge(
            IERC20(address(usdc)), ILayerZeroEndpointV2(address(lzEndpoint)), LOCAL_EID, admin
        );
        vm.prank(admin);
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.InvalidThreshold.selector, uint8(0), uint16(2)));
        fresh.setSigners(s, 0);
    }

    function test_removeSignerLowersThresholdIfTooHigh() public {
        vm.prank(admin);
        bridge.setThreshold(3);
        assertEq(bridge.threshold(), 3);
        vm.prank(admin);
        bridge.removeSigner(signer1);
        assertEq(bridge.threshold(), 2); // auto-lowered to signerCount
        assertEq(bridge.signerCount(), 2);
    }

    function test_withdrawNativeMovesEth() public {
        uint256 startBal = address(this).balance;
        vm.prank(admin);
        bridge.withdrawNative(payable(address(this)), 0.5 ether);
        assertEq(address(this).balance, startBal + 0.5 ether);
    }

    receive() external payable {}

    // ── bridgeOut ──────────────────────────────────────────────────────

    function _fundBank(uint256 amount) internal {
        usdc.mint(address(bank), amount);
        vm.prank(address(bank));
        usdc.approve(address(bridge), amount);
    }

    function test_bridgeOutHappyPath() public {
        uint256 amount = 50_000e6;
        address destReserve = address(0xDE57);
        _fundBank(amount);

        uint256 ethBefore = address(bridge).balance;
        vm.prank(address(bank));
        bytes32 messageId = bridge.bridgeOut(TEMPO_CHAIN_ID, amount, destReserve);

        // USDC is now custodied in the adapter — released later by executeRelease on dest.
        assertEq(usdc.balanceOf(address(bank)), 0);
        assertEq(usdc.balanceOf(address(bridge)), amount);

        // LZ endpoint received the right send.
        (uint32 dstEid, bytes32 receiver, bytes memory message, uint256 value, address sender) = lzEndpoint.lastSend();
        assertEq(dstEid, TEMPO_EID);
        assertEq(receiver, remoteAdapterBytes32);
        (uint256 amt, address recip, bytes32 mid) = abi.decode(message, (uint256, address, bytes32));
        assertEq(amt, amount);
        assertEq(recip, destReserve);
        assertEq(mid, messageId);
        assertEq(value, 0.001 ether); // fee paid from adapter balance
        assertEq(sender, address(bridge));
        assertEq(address(bridge).balance, ethBefore - 0.001 ether);

        bytes32 expected = keccak256(abi.encode(bytes32("TEMPO_MULTISIG"), LOCAL_EID, uint64(1)));
        assertEq(messageId, expected);
    }

    function test_bridgeOutRevertsForNonBank() public {
        _fundBank(1e6);
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.OnlyBank.selector, other));
        vm.prank(other);
        bridge.bridgeOut(TEMPO_CHAIN_ID, 1e6, address(0xDE57));
    }

    function test_bridgeOutRevertsOnZeroAmount() public {
        vm.prank(address(bank));
        vm.expectRevert(TempoReserveBridge.ZeroAmount.selector);
        bridge.bridgeOut(TEMPO_CHAIN_ID, 0, address(0xDE57));
    }

    function test_bridgeOutRevertsOnUnconfiguredChain() public {
        vm.prank(address(bank));
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.ChainNotConfigured.selector, uint64(99)));
        bridge.bridgeOut(99, 1e6, address(0xDE57));
    }

    function test_bridgeOutRevertsOnRemoteAdapterUnset() public {
        vm.prank(admin);
        bridge.setChainEid(uint64(123), uint32(456));
        vm.prank(address(bank));
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.RemoteAdapterNotConfigured.selector, uint32(456)));
        bridge.bridgeOut(uint64(123), 1e6, address(0xDE57));
    }

    function test_bridgeOutRevertsOnInsufficientNativeBalance() public {
        uint256 amount = 1e6;
        _fundBank(amount);
        vm.prank(admin);
        bridge.withdrawNative(payable(address(this)), address(bridge).balance);

        vm.prank(address(bank));
        vm.expectRevert(
            abi.encodeWithSelector(
                TempoReserveBridge.InsufficientNativeBalance.selector, uint256(0.001 ether), uint256(0)
            )
        );
        bridge.bridgeOut(TEMPO_CHAIN_ID, amount, address(0xDE57));
    }

    function test_bridgeInAlwaysReverts() public {
        vm.expectRevert(TempoReserveBridge.UseExecuteRelease.selector);
        bridge.bridgeIn(hex"01", hex"02");
    }

    function test_quoteMatchesEndpointFee() public view {
        uint256 q = bridge.quoteBridgeOut(TEMPO_CHAIN_ID, 1e6, address(0xDE57));
        assertEq(q, 0.001 ether);
    }

    // ── lzReceive ──────────────────────────────────────────────────────

    function _lzDeliver(uint32 srcEid, bytes32 sender, uint256 amount, address recipient, bytes32 messageId) internal {
        Origin memory origin = Origin({srcEid: srcEid, sender: sender, nonce: 1});
        bytes memory payload = abi.encode(amount, recipient, messageId);
        vm.prank(address(lzEndpoint));
        bridge.lzReceive(origin, bytes32(0), payload, address(0), "");
    }

    function test_lzReceiveQueuesPendingRelease() public {
        bytes32 mid = keccak256("test-message");
        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, 100e6, address(0xCAFE), mid);

        (uint64 src, uint256 amt, address recip, bool exists, bool executed) = bridge.pendingReleases(mid);
        assertEq(src, TEMPO_CHAIN_ID);
        assertEq(amt, 100e6);
        assertEq(recip, address(0xCAFE));
        assertTrue(exists);
        assertFalse(executed);
    }

    function test_lzReceiveRevertsForNonEndpoint() public {
        Origin memory origin = Origin({srcEid: TEMPO_EID, sender: remoteAdapterBytes32, nonce: 1});
        bytes memory payload = abi.encode(uint256(1), address(0xCAFE), bytes32("m"));
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.OnlyEndpoint.selector, other));
        vm.prank(other);
        bridge.lzReceive(origin, bytes32(0), payload, address(0), "");
    }

    function test_lzReceiveRevertsOnUnknownEid() public {
        bytes32 mid = bytes32("m");
        Origin memory origin = Origin({srcEid: uint32(99), sender: remoteAdapterBytes32, nonce: 1});
        bytes memory payload = abi.encode(uint256(1), address(0xCAFE), mid);
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.EidNotConfigured.selector, uint32(99)));
        vm.prank(address(lzEndpoint));
        bridge.lzReceive(origin, bytes32(0), payload, address(0), "");
    }

    function test_lzReceiveRevertsOnUnexpectedSender() public {
        bytes32 bad = bytes32(uint256(uint160(0xDEAD)));
        Origin memory origin = Origin({srcEid: TEMPO_EID, sender: bad, nonce: 1});
        bytes memory payload = abi.encode(uint256(1), address(0xCAFE), bytes32("m"));
        vm.expectRevert(
            abi.encodeWithSelector(TempoReserveBridge.UnexpectedSender.selector, remoteAdapterBytes32, bad)
        );
        vm.prank(address(lzEndpoint));
        bridge.lzReceive(origin, bytes32(0), payload, address(0), "");
    }

    function test_lzReceiveIsIdempotentOnDuplicate() public {
        bytes32 mid = bytes32("dup");
        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, 100e6, address(0xCAFE), mid);
        // Second delivery is a silent no-op (LZ retries are safe).
        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, 100e6, address(0xCAFE), mid);
        (,,,, bool executed) = bridge.pendingReleases(mid);
        assertFalse(executed);
    }

    // ── executeRelease ─────────────────────────────────────────────────

    function _domainSeparator() internal view returns (bytes32) {
        return keccak256(
            abi.encode(
                keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
                keccak256(bytes("TempoReserveBridge")),
                keccak256(bytes("1")),
                block.chainid,
                address(bridge)
            )
        );
    }

    function _digest(uint64 sourceChainId, uint256 amount, address recipient, bytes32 messageId)
        internal
        view
        returns (bytes32)
    {
        bytes32 structHash =
            keccak256(abi.encode(bridge.RELEASE_TYPEHASH(), sourceChainId, amount, recipient, messageId));
        return keccak256(abi.encodePacked("\x19\x01", _domainSeparator(), structHash));
    }

    function _sign(uint256 sk, bytes32 digest) internal pure returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(sk, digest);
        return abi.encodePacked(r, s, v);
    }

    // Sort the (signer, signature) pairs by signer address ascending so the contract's strict
    // ordering check accepts them.
    function _sortedSigs(uint256[2] memory sks, address[2] memory signers, bytes32 digest)
        internal
        pure
        returns (bytes[] memory)
    {
        bytes[] memory out = new bytes[](2);
        if (signers[0] < signers[1]) {
            out[0] = _sign(sks[0], digest);
            out[1] = _sign(sks[1], digest);
        } else {
            out[0] = _sign(sks[1], digest);
            out[1] = _sign(sks[0], digest);
        }
        return out;
    }

    function test_executeReleaseHappyPath() public {
        bytes32 mid = bytes32("good");
        uint256 amount = 250e6;
        address recipient = address(0xCAFE);

        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, amount, recipient, mid);
        // The adapter needs USDC on hand to release (mirrors the production state where USDC is
        // pre-positioned on Tempo by the issuer or earlier bridges).
        usdc.mint(address(bridge), amount);

        bytes32 digest = _digest(TEMPO_CHAIN_ID, amount, recipient, mid);
        bytes[] memory sigs = _sortedSigs([sk1, sk2], [signer1, signer2], digest);

        bridge.executeRelease(mid, sigs);

        // Bank received the USDC and recorded the credit.
        assertEq(usdc.balanceOf(address(bank)), amount);
        assertEq(bank.completionsLength(), 1);
        (uint64 src, uint256 amt, bytes32 recordedMid) = bank.completions(0);
        assertEq(src, TEMPO_CHAIN_ID);
        assertEq(amt, amount);
        assertEq(recordedMid, mid);

        // pending.executed is now true.
        (,,,, bool executed) = bridge.pendingReleases(mid);
        assertTrue(executed);
    }

    function test_executeReleaseRevertsIfNotPending() public {
        bytes[] memory sigs = new bytes[](2);
        sigs[0] = hex"00";
        sigs[1] = hex"00";
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.MessageNotPending.selector, bytes32("missing")));
        bridge.executeRelease(bytes32("missing"), sigs);
    }

    function test_executeReleaseRevertsOnReplay() public {
        bytes32 mid = bytes32("replay");
        uint256 amount = 1e6;
        address recipient = address(0xCAFE);
        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, amount, recipient, mid);
        usdc.mint(address(bridge), amount);

        bytes32 digest = _digest(TEMPO_CHAIN_ID, amount, recipient, mid);
        bytes[] memory sigs = _sortedSigs([sk1, sk2], [signer1, signer2], digest);

        bridge.executeRelease(mid, sigs);
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.MessageAlreadyExecuted.selector, mid));
        bridge.executeRelease(mid, sigs);
    }

    function test_executeReleaseRevertsOnInsufficientSignatures() public {
        bytes32 mid = bytes32("short");
        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, 1e6, address(0xCAFE), mid);
        bytes[] memory sigs = new bytes[](1);
        sigs[0] = _sign(sk1, _digest(TEMPO_CHAIN_ID, 1e6, address(0xCAFE), mid));
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.NotEnoughSignatures.selector, uint8(1), uint8(2)));
        bridge.executeRelease(mid, sigs);
    }

    function test_executeReleaseRevertsOnUnknownSigner() public {
        bytes32 mid = bytes32("foreign");
        uint256 amount = 1e6;
        address recipient = address(0xCAFE);
        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, amount, recipient, mid);
        usdc.mint(address(bridge), amount);

        bytes32 digest = _digest(TEMPO_CHAIN_ID, amount, recipient, mid);
        uint256 sk_outsider = 0xBADBAD;
        bytes[] memory sigs = new bytes[](2);
        // Provide signer1 + an outsider, sorted by address ascending. We sort the bytes by signer
        // address.
        address outsider = vm.addr(sk_outsider);
        if (signer1 < outsider) {
            sigs[0] = _sign(sk1, digest);
            sigs[1] = _sign(sk_outsider, digest);
        } else {
            sigs[0] = _sign(sk_outsider, digest);
            sigs[1] = _sign(sk1, digest);
        }
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.UnknownSigner.selector, outsider));
        bridge.executeRelease(mid, sigs);
    }

    function test_executeReleaseRevertsOnDuplicateOrUnordered() public {
        bytes32 mid = bytes32("dup-sig");
        uint256 amount = 1e6;
        address recipient = address(0xCAFE);
        _lzDeliver(TEMPO_EID, remoteAdapterBytes32, amount, recipient, mid);
        usdc.mint(address(bridge), amount);

        bytes32 digest = _digest(TEMPO_CHAIN_ID, amount, recipient, mid);
        // Two valid signatures from the same signer — the second fails the strict-ascending check.
        bytes[] memory sigs = new bytes[](2);
        sigs[0] = _sign(sk1, digest);
        sigs[1] = _sign(sk1, digest);
        vm.expectRevert(abi.encodeWithSelector(TempoReserveBridge.DuplicateSigner.selector, signer1));
        bridge.executeRelease(mid, sigs);
    }

    function test_bridgeTypeIsTempoMultisig() public view {
        assertEq(bridge.bridgeType(), bytes32("TEMPO_MULTISIG"));
    }
}
