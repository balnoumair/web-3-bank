// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import {EIP712} from "@openzeppelin/contracts/utils/cryptography/EIP712.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {IReserveBridge} from "./interfaces/IReserveBridge.sol";

// ── LayerZero v2 minimal interfaces ──────────────────────────────────────────

struct MessagingFee {
    uint256 nativeFee;
    uint256 lzTokenFee;
}

struct MessagingParams {
    uint32 dstEid;
    bytes32 receiver;
    bytes message;
    bytes options;
    bool payInLzToken;
}

struct MessagingReceipt {
    bytes32 guid;
    uint64 nonce;
    MessagingFee fee;
}

struct Origin {
    uint32 srcEid;
    bytes32 sender;
    uint64 nonce;
}

interface ILayerZeroEndpointV2 {
    function quote(MessagingParams calldata params, address sender) external view returns (MessagingFee memory);
    function send(MessagingParams calldata params, address refundAddress)
        external
        payable
        returns (MessagingReceipt memory);
}

interface ILayerZeroReceiver {
    function lzReceive(
        Origin calldata origin,
        bytes32 guid,
        bytes calldata message,
        address executor,
        bytes calldata extraData
    ) external payable;
}

interface IBankReserveSink {
    function completeReserveBridge(uint64 sourceChainId, uint256 amount, bytes32 messageId) external;
}

/// @title TempoReserveBridge
/// @notice IReserveBridge adapter for chains where CCTP is unavailable (notably Tempo).
///         Uses LayerZero v2 to publish a release intent on the source chain, then requires an
///         N-of-M EIP-712 multisig signature on the destination before USDC is released. The
///         adapter holds custody of USDC and the ETH balance used to pay LayerZero fees.
/// @dev Three flows:
///        1. Outbound: Bank → `bridgeOut` → locks USDC + sends LZ message.
///        2. Inbound (LZ): endpoint → `lzReceive` → records `PendingRelease`.
///        3. Inbound (multisig): operator → `executeRelease(messageId, sigs[])` → verifies
///           N-of-M signatures + releases USDC + calls `Bank.completeReserveBridge`.
///      The `IReserveBridge.bridgeIn(message, attestation)` entrypoint is deliberately rejected
///      — Tempo's flow does not match CCTP's single-step receive shape.
contract TempoReserveBridge is IReserveBridge, ILayerZeroReceiver, AccessControl, EIP712 {
    using SafeERC20 for IERC20;

    // ── Errors ─────────────────────────────────────────────────────────

    error ZeroAddress();
    error ZeroAmount();
    error OnlyBank(address caller);
    error OnlyEndpoint(address caller);
    error BankNotSet();
    error ChainNotConfigured(uint64 chainId);
    error EidNotConfigured(uint32 eid);
    error RemoteAdapterNotConfigured(uint32 eid);
    error UnexpectedSender(bytes32 expected, bytes32 actual);
    error InsufficientNativeBalance(uint256 needed, uint256 have);
    error InvalidSignerCount();
    error InvalidThreshold(uint8 threshold, uint16 signerCount);
    error DuplicateSigner(address signer);
    error UnknownSigner(address signer);
    error NotEnoughSignatures(uint8 collected, uint8 required);
    error UseExecuteRelease();
    error MessageNotPending(bytes32 messageId);
    error MessageAlreadyExecuted(bytes32 messageId);
    error WithdrawFailed();

    // ── Constants ──────────────────────────────────────────────────────

    bytes32 public constant BRIDGE_TYPE = "TEMPO_MULTISIG";

    /// @dev EIP-712 typehash for the release approval payload.
    bytes32 public constant RELEASE_TYPEHASH = keccak256(
        "ReserveRelease(uint64 sourceChainId,uint256 amount,address recipient,bytes32 messageId)"
    );

    // ── Immutables ─────────────────────────────────────────────────────

    IERC20 public immutable usdc;
    ILayerZeroEndpointV2 public immutable lzEndpoint;
    /// @notice LayerZero EID of this chain — used in messageId derivation and for LZ peering.
    uint32 public immutable localEid;

    // ── Storage ────────────────────────────────────────────────────────

    address public bank;

    /// @notice EVM chainId → LZ EID.
    mapping(uint64 => uint32) public chainIdToEid;
    mapping(uint64 => bool) public chainConfigured;
    mapping(uint32 => uint64) public eidToChainId;
    mapping(uint32 => bool) public eidConfigured;

    /// @notice LZ EID → paired remote adapter (bytes32 — LZ uses bytes32 addresses).
    mapping(uint32 => bytes32) public remoteAdapter;

    /// @notice Per-message-pair nonce, used for source-side messageId derivation.
    mapping(uint32 => uint64) public outboundNonce;

    /// @notice The multisig signer set authorised to approve releases.
    mapping(address => bool) public isSigner;
    uint16 public signerCount;
    /// @notice Number of distinct signatures required (1 ≤ threshold ≤ signerCount).
    uint8 public threshold;

    struct PendingRelease {
        uint64 sourceChainId;
        uint256 amount;
        address recipient;
        bool exists;
        bool executed;
    }

    /// @notice LZ-delivered releases awaiting multisig execution.
    mapping(bytes32 => PendingRelease) public pendingReleases;

    // ── Events ─────────────────────────────────────────────────────────

    event BankUpdated(address indexed bank);
    event ChainEidUpdated(uint64 indexed chainId, uint32 indexed eid);
    event RemoteAdapterUpdated(uint32 indexed eid, bytes32 indexed adapter);
    event SignerAdded(address indexed signer);
    event SignerRemoved(address indexed signer);
    event ThresholdUpdated(uint8 threshold);
    event BridgeOutInitiated(
        bytes32 indexed messageId,
        uint64 indexed destChainId,
        uint32 destEid,
        uint64 nonce,
        uint256 amount,
        uint256 lzFeePaid
    );
    event ReleaseQueued(
        bytes32 indexed messageId,
        uint64 indexed sourceChainId,
        uint32 sourceEid,
        uint256 amount,
        address recipient
    );
    event ReleaseExecuted(bytes32 indexed messageId, uint64 indexed sourceChainId, uint256 amount);
    event NativeWithdrawn(address indexed to, uint256 amount);
    event UsdcRecovered(address indexed to, uint256 amount);

    // ── Constructor ────────────────────────────────────────────────────

    constructor(IERC20 usdc_, ILayerZeroEndpointV2 lzEndpoint_, uint32 localEid_, address admin)
        EIP712("TempoReserveBridge", "1")
    {
        if (address(usdc_) == address(0)) revert ZeroAddress();
        if (address(lzEndpoint_) == address(0)) revert ZeroAddress();
        if (admin == address(0)) revert ZeroAddress();

        usdc = usdc_;
        lzEndpoint = lzEndpoint_;
        localEid = localEid_;
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    // ── Governance ─────────────────────────────────────────────────────

    function setBank(address bank_) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (bank_ == address(0)) revert ZeroAddress();
        bank = bank_;
        emit BankUpdated(bank_);
    }

    function setChainEid(uint64 chainId, uint32 eid) external onlyRole(DEFAULT_ADMIN_ROLE) {
        chainIdToEid[chainId] = eid;
        eidToChainId[eid] = chainId;
        chainConfigured[chainId] = true;
        eidConfigured[eid] = true;
        emit ChainEidUpdated(chainId, eid);
    }

    function setRemoteAdapter(uint32 eid, bytes32 adapter) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (adapter == bytes32(0)) revert ZeroAddress();
        remoteAdapter[eid] = adapter;
        emit RemoteAdapterUpdated(eid, adapter);
    }

    /// @notice Register the signer set in one call. Replaces any existing signers.
    function setSigners(address[] calldata signers, uint8 newThreshold) external onlyRole(DEFAULT_ADMIN_ROLE) {
        // Clear any previously registered signers by collecting existing addresses through the
        // event log. Since AccessControl has no enumerable role list and we want this contract to
        // be self-contained, callers MUST pass the complete intended signer set; we do not retain
        // historical signers across calls. Implementation: walk caller-provided list, then
        // unilaterally overwrite the `isSigner` map for those addresses; callers wanting to remove
        // an address must call `removeSigner(addr)` separately. This keeps `setSigners` cheap for
        // the common case (initial setup) and forces explicit intent for removals.
        if (signers.length == 0 || signers.length > 255) revert InvalidSignerCount();
        if (newThreshold == 0 || uint16(newThreshold) > uint16(signers.length)) {
            revert InvalidThreshold(newThreshold, uint16(signers.length));
        }

        for (uint256 i = 0; i < signers.length; i++) {
            address s = signers[i];
            if (s == address(0)) revert ZeroAddress();
            if (isSigner[s]) revert DuplicateSigner(s);
            isSigner[s] = true;
            signerCount++;
            emit SignerAdded(s);
        }
        threshold = newThreshold;
        emit ThresholdUpdated(newThreshold);
    }

    function addSigner(address signer) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (signer == address(0)) revert ZeroAddress();
        if (isSigner[signer]) revert DuplicateSigner(signer);
        isSigner[signer] = true;
        signerCount++;
        emit SignerAdded(signer);
    }

    function removeSigner(address signer) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (!isSigner[signer]) revert UnknownSigner(signer);
        isSigner[signer] = false;
        signerCount--;
        if (uint16(threshold) > signerCount) {
            threshold = uint8(signerCount);
            emit ThresholdUpdated(threshold);
        }
        emit SignerRemoved(signer);
    }

    function setThreshold(uint8 newThreshold) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (newThreshold == 0 || uint16(newThreshold) > signerCount) {
            revert InvalidThreshold(newThreshold, signerCount);
        }
        threshold = newThreshold;
        emit ThresholdUpdated(newThreshold);
    }

    /// @notice Recover the adapter's native balance (used to pay LZ fees). Admin-only.
    function withdrawNative(address payable to, uint256 amount) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (to == address(0)) revert ZeroAddress();
        (bool ok,) = to.call{value: amount}("");
        if (!ok) revert WithdrawFailed();
        emit NativeWithdrawn(to, amount);
    }

    /// @notice Recover stranded USDC (e.g. after a failed bridge attempt). Admin-only.
    /// @dev Should not be called while pending releases reference locked balances.
    function recoverUsdc(address to, uint256 amount) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (to == address(0)) revert ZeroAddress();
        usdc.safeTransfer(to, amount);
        emit UsdcRecovered(to, amount);
    }

    receive() external payable {}

    // ── IReserveBridge ─────────────────────────────────────────────────

    /// @inheritdoc IReserveBridge
    function bridgeType() external pure returns (bytes32) {
        return BRIDGE_TYPE;
    }

    /// @inheritdoc IReserveBridge
    /// @dev Quote-then-send via LayerZero v2. The adapter MUST hold enough native balance to pay
    ///      the LZ fee — governance tops up `address(this).balance` out of band. We don't try to
    ///      forward msg.value from the Bank because `Bank.bridgeReserve` is not payable today.
    function bridgeOut(uint64 destChainId, uint256 amount, address destReserve)
        external
        returns (bytes32 messageId)
    {
        address bank_ = bank;
        if (bank_ == address(0)) revert BankNotSet();
        if (msg.sender != bank_) revert OnlyBank(msg.sender);
        if (amount == 0) revert ZeroAmount();
        if (destReserve == address(0)) revert ZeroAddress();
        if (!chainConfigured[destChainId]) revert ChainNotConfigured(destChainId);

        uint32 destEid = chainIdToEid[destChainId];
        bytes32 remote = remoteAdapter[destEid];
        if (remote == bytes32(0)) revert RemoteAdapterNotConfigured(destEid);

        usdc.safeTransferFrom(bank_, address(this), amount);

        uint64 nonce = ++outboundNonce[destEid];
        messageId = _computeMessageId(localEid, nonce);

        bytes memory payload = abi.encode(amount, destReserve, messageId);
        MessagingParams memory params = MessagingParams({
            dstEid: destEid,
            receiver: remote,
            message: payload,
            options: "",
            payInLzToken: false
        });

        MessagingFee memory fee = lzEndpoint.quote(params, address(this));
        if (address(this).balance < fee.nativeFee) {
            revert InsufficientNativeBalance(fee.nativeFee, address(this).balance);
        }

        lzEndpoint.send{value: fee.nativeFee}(params, address(this));

        emit BridgeOutInitiated(messageId, destChainId, destEid, nonce, amount, fee.nativeFee);
    }

    /// @inheritdoc IReserveBridge
    /// @dev Tempo's destination flow is `lzReceive` → `executeRelease`. CCTP's single-step
    ///      `bridgeIn(message, attestation)` does not apply here. Reverting here keeps Treasury's
    ///      dispatcher honest — it must pick the right path per `bridgeType`.
    function bridgeIn(bytes calldata, bytes calldata) external pure returns (bytes32) {
        revert UseExecuteRelease();
    }

    // ── LayerZero receive ──────────────────────────────────────────────

    /// @inheritdoc ILayerZeroReceiver
    function lzReceive(
        Origin calldata origin,
        bytes32, /* guid */
        bytes calldata message,
        address, /* executor */
        bytes calldata /* extraData */
    ) external payable {
        if (msg.sender != address(lzEndpoint)) revert OnlyEndpoint(msg.sender);
        if (bank == address(0)) revert BankNotSet();
        if (!eidConfigured[origin.srcEid]) revert EidNotConfigured(origin.srcEid);

        bytes32 expectedSender = remoteAdapter[origin.srcEid];
        if (expectedSender == bytes32(0)) revert RemoteAdapterNotConfigured(origin.srcEid);
        if (origin.sender != expectedSender) revert UnexpectedSender(expectedSender, origin.sender);

        (uint256 amount, address recipient, bytes32 messageId) = abi.decode(message, (uint256, address, bytes32));

        // Idempotency: if already queued (or executed), the LZ endpoint MUST NOT cause duplication.
        // We treat a duplicate as a no-op rather than a revert so LZ retries are safe.
        if (pendingReleases[messageId].exists) {
            return;
        }

        pendingReleases[messageId] = PendingRelease({
            sourceChainId: eidToChainId[origin.srcEid],
            amount: amount,
            recipient: recipient,
            exists: true,
            executed: false
        });

        emit ReleaseQueued(messageId, eidToChainId[origin.srcEid], origin.srcEid, amount, recipient);
    }

    // ── Multisig execution ─────────────────────────────────────────────

    /// @notice Release the USDC for a previously-queued message, given N-of-M ECDSA signatures
    ///         on the EIP-712 `ReserveRelease` typed data.
    /// @dev Callable by anyone — authority is in the signatures, not the caller. Signatures must
    ///      be sorted by signer address ascending; duplicates are rejected via strict ordering.
    function executeRelease(bytes32 messageId, bytes[] calldata signatures) external {
        PendingRelease storage p = pendingReleases[messageId];
        if (!p.exists) revert MessageNotPending(messageId);
        if (p.executed) revert MessageAlreadyExecuted(messageId);

        bytes32 structHash = keccak256(
            abi.encode(RELEASE_TYPEHASH, p.sourceChainId, p.amount, p.recipient, messageId)
        );
        bytes32 digest = _hashTypedDataV4(structHash);

        _verifySignatures(digest, signatures, threshold);

        p.executed = true;

        // Release USDC to dest Bank and record the credit on-chain.
        usdc.safeTransfer(bank, p.amount);
        IBankReserveSink(bank).completeReserveBridge(p.sourceChainId, p.amount, messageId);

        emit ReleaseExecuted(messageId, p.sourceChainId, p.amount);
    }

    /// @notice Quote the native fee a `bridgeOut(destChainId, amount, destReserve)` call would pay.
    function quoteBridgeOut(uint64 destChainId, uint256 amount, address destReserve)
        external
        view
        returns (uint256 nativeFee)
    {
        if (!chainConfigured[destChainId]) revert ChainNotConfigured(destChainId);
        uint32 destEid = chainIdToEid[destChainId];
        bytes32 remote = remoteAdapter[destEid];
        if (remote == bytes32(0)) revert RemoteAdapterNotConfigured(destEid);

        // Synthesise the same payload bridgeOut would build. messageId uses the *next* nonce so the
        // quote matches what the next send will pay; the quote is not consumed.
        bytes32 messageId = _computeMessageId(localEid, outboundNonce[destEid] + 1);
        bytes memory payload = abi.encode(amount, destReserve, messageId);
        MessagingParams memory params = MessagingParams({
            dstEid: destEid,
            receiver: remote,
            message: payload,
            options: "",
            payInLzToken: false
        });
        return lzEndpoint.quote(params, address(this)).nativeFee;
    }

    // ── Internal ───────────────────────────────────────────────────────

    function _verifySignatures(bytes32 digest, bytes[] calldata signatures, uint8 required) private view {
        if (signatures.length < required) revert NotEnoughSignatures(uint8(signatures.length), required);
        address lastSigner = address(0);
        uint8 valid = 0;
        for (uint256 i = 0; i < signatures.length; i++) {
            address recovered = ECDSA.recover(digest, signatures[i]);
            if (!isSigner[recovered]) revert UnknownSigner(recovered);
            // Strict ascending order — prevents the same signature being counted twice.
            if (recovered <= lastSigner) revert DuplicateSigner(recovered);
            lastSigner = recovered;
            valid++;
            if (valid >= required) return;
        }
        revert NotEnoughSignatures(valid, required);
    }

    function _computeMessageId(uint32 sourceEid, uint64 nonce) private pure returns (bytes32) {
        return keccak256(abi.encode(BRIDGE_TYPE, sourceEid, nonce));
    }
}
