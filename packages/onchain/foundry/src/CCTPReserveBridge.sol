// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {IReserveBridge} from "./interfaces/IReserveBridge.sol";

/// @title ITokenMessenger
/// @notice Minimal local view of Circle's CCTP v1 TokenMessenger. Only the functions we call are declared.
interface ITokenMessenger {
    function depositForBurnWithCaller(
        uint256 amount,
        uint32 destinationDomain,
        bytes32 mintRecipient,
        address burnToken,
        bytes32 destinationCaller
    ) external returns (uint64 nonce);
}

/// @title IMessageTransmitter
/// @notice Minimal local view of Circle's CCTP v1 MessageTransmitter. Only the functions we call are declared.
interface IMessageTransmitter {
    function receiveMessage(bytes calldata message, bytes calldata attestation) external returns (bool success);
}

/// @title IBankReserveSink
/// @notice Minimal local view of the Bank's inbound reserve-bridge entrypoint.
interface IBankReserveSink {
    function completeReserveBridge(uint64 sourceChainId, uint256 amount, bytes32 messageId) external;
}

/// @title CCTPReserveBridge
/// @notice IReserveBridge adapter backed by Circle's CCTP v1. Burns USDC on the source chain and mints
///         fresh USDC on the destination chain to the destination Bank contract.
/// @dev One instance is deployed per chain alongside its Bank. Governance pairs adapters across chains
///      via `setRemoteAdapter`. Outbound burns set CCTP's `destinationCaller` to the paired remote
///      adapter so that only that adapter may relay `receiveMessage`, preserving the audit path.
contract CCTPReserveBridge is IReserveBridge, AccessControl {
    using SafeERC20 for IERC20;

    // ── Errors ─────────────────────────────────────────────────────────

    error ZeroAddress();
    error ZeroAmount();
    error OnlyBank(address caller);
    error BankNotSet();
    error ChainNotConfigured(uint64 chainId);
    error DomainNotConfigured(uint32 domain);
    error RemoteAdapterNotConfigured(uint32 domain);
    error MessageTooShort(uint256 length);
    error ReceiveMessageFailed();
    error UnexpectedSender(bytes32 expected, bytes32 actual);

    // ── Constants ──────────────────────────────────────────────────────

    /// @dev Identifier returned by `bridgeType()` for Treasury audit records.
    bytes32 public constant BRIDGE_TYPE = "CCTP";

    /// @dev CCTP v1 message header is fixed at 116 bytes:
    ///      version(4) | sourceDomain(4) | destDomain(4) | nonce(8) | sender(32) | recipient(32) | destCaller(32).
    uint256 private constant HEADER_LEN = 116;

    /// @dev CCTP v1 burn-message body is fixed at 132 bytes:
    ///      bodyVersion(4) | burnToken(32) | mintRecipient(32) | amount(32) | messageSender(32).
    uint256 private constant BURN_BODY_LEN = 132;

    // ── Immutables ─────────────────────────────────────────────────────

    /// @notice USDC token contract on this chain.
    IERC20 public immutable usdc;

    /// @notice Circle CCTP TokenMessenger on this chain (handles depositForBurn).
    ITokenMessenger public immutable tokenMessenger;

    /// @notice Circle CCTP MessageTransmitter on this chain (handles receiveMessage).
    IMessageTransmitter public immutable messageTransmitter;

    /// @notice CCTP domain identifier for this chain (e.g. Ethereum=0, Avalanche=1, Base=6).
    uint32 public immutable localDomain;

    // ── Storage ────────────────────────────────────────────────────────

    /// @notice The Bank contract this adapter serves. Outbound: only this address may call `bridgeOut`.
    ///         Inbound: this is the recipient passed to `completeReserveBridge`.
    address public bank;

    /// @notice EVM chainId → CCTP domain.
    mapping(uint64 => uint32) public chainIdToDomain;

    /// @notice True iff `setChainDomain` has registered the chainId. Disambiguates domain 0 (Ethereum).
    mapping(uint64 => bool) public chainConfigured;

    /// @notice CCTP domain → EVM chainId (reverse of `chainIdToDomain`, written together).
    mapping(uint32 => uint64) public domainToChainId;

    /// @notice True iff `setChainDomain` has registered the domain. Disambiguates chainId 0.
    mapping(uint32 => bool) public domainConfigured;

    /// @notice CCTP domain → paired CCTPReserveBridge address on that domain. Used as `destinationCaller`
    ///         on outbound burns and as the expected `sender` on inbound messages.
    mapping(uint32 => address) public remoteAdapter;

    // ── Events ─────────────────────────────────────────────────────────

    event BankUpdated(address indexed bank);
    event ChainDomainUpdated(uint64 indexed chainId, uint32 indexed domain);
    event RemoteAdapterUpdated(uint32 indexed domain, address indexed adapter);
    event BridgeOutInitiated(
        bytes32 indexed messageId, uint64 indexed destChainId, uint32 destDomain, uint64 nonce, uint256 amount
    );
    event BridgeInCompleted(
        bytes32 indexed messageId, uint64 indexed sourceChainId, uint32 sourceDomain, uint64 nonce, uint256 amount
    );

    // ── Constructor ────────────────────────────────────────────────────

    constructor(
        IERC20 usdc_,
        ITokenMessenger tokenMessenger_,
        IMessageTransmitter messageTransmitter_,
        uint32 localDomain_,
        address admin
    ) {
        if (address(usdc_) == address(0)) revert ZeroAddress();
        if (address(tokenMessenger_) == address(0)) revert ZeroAddress();
        if (address(messageTransmitter_) == address(0)) revert ZeroAddress();
        if (admin == address(0)) revert ZeroAddress();

        usdc = usdc_;
        tokenMessenger = tokenMessenger_;
        messageTransmitter = messageTransmitter_;
        localDomain = localDomain_;
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    // ── Governance ─────────────────────────────────────────────────────

    /// @notice Set the Bank contract this adapter serves.
    function setBank(address bank_) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (bank_ == address(0)) revert ZeroAddress();
        bank = bank_;
        emit BankUpdated(bank_);
    }

    /// @notice Register the CCTP domain for an EVM chainId. Writes both directions.
    function setChainDomain(uint64 chainId, uint32 domain) external onlyRole(DEFAULT_ADMIN_ROLE) {
        chainIdToDomain[chainId] = domain;
        domainToChainId[domain] = chainId;
        chainConfigured[chainId] = true;
        domainConfigured[domain] = true;
        emit ChainDomainUpdated(chainId, domain);
    }

    /// @notice Register the paired CCTPReserveBridge on a remote domain.
    function setRemoteAdapter(uint32 domain, address adapter) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (adapter == address(0)) revert ZeroAddress();
        remoteAdapter[domain] = adapter;
        emit RemoteAdapterUpdated(domain, adapter);
    }

    // ── IReserveBridge ─────────────────────────────────────────────────

    /// @inheritdoc IReserveBridge
    function bridgeType() external pure returns (bytes32) {
        return BRIDGE_TYPE;
    }

    /// @inheritdoc IReserveBridge
    /// @dev Called by Bank from inside `bridgeReserve`. The Bank approved `amount` to this adapter
    ///      immediately before the call; we pull, then approve TokenMessenger, then burn.
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

        uint32 destDomain = chainIdToDomain[destChainId];
        address remote = remoteAdapter[destDomain];
        if (remote == address(0)) revert RemoteAdapterNotConfigured(destDomain);

        usdc.safeTransferFrom(bank_, address(this), amount);
        usdc.forceApprove(address(tokenMessenger), amount);

        bytes32 mintRecipient = _addressToBytes32(destReserve);
        bytes32 destinationCaller = _addressToBytes32(remote);

        uint64 nonce = tokenMessenger.depositForBurnWithCaller(
            amount, destDomain, mintRecipient, address(usdc), destinationCaller
        );

        messageId = _computeMessageId(localDomain, nonce);
        emit BridgeOutInitiated(messageId, destChainId, destDomain, nonce, amount);
    }

    /// @inheritdoc IReserveBridge
    /// @dev Anyone may call this — CCTP's `destinationCaller` field already restricts who can pass the
    ///      attestation to MessageTransmitter, so the only thing an external caller can do is pay gas.
    function bridgeIn(bytes calldata message, bytes calldata attestation) external returns (bytes32 messageId) {
        address bank_ = bank;
        if (bank_ == address(0)) revert BankNotSet();
        if (message.length < HEADER_LEN + BURN_BODY_LEN) revert MessageTooShort(message.length);

        uint32 sourceDomain = _readUint32(message, 4);
        uint64 nonce = _readUint64(message, 12);
        uint256 amount = _readUint256(message, HEADER_LEN + 4 + 32 + 32);
        // CCTP burn-message body's `messageSender` (final 32 bytes of body) is the caller of
        // `depositForBurn` on the source — i.e. the paired remote adapter. The header's `sender`
        // field instead holds the source TokenMessenger, which is the same address for everyone
        // on a given chain and therefore not useful for adapter authentication.
        bytes32 bodySender = _readBytes32(message, HEADER_LEN + 4 + 32 + 32 + 32);

        if (!domainConfigured[sourceDomain]) revert DomainNotConfigured(sourceDomain);
        address remote = remoteAdapter[sourceDomain];
        if (remote == address(0)) revert RemoteAdapterNotConfigured(sourceDomain);

        bytes32 expectedSender = _addressToBytes32(remote);
        if (bodySender != expectedSender) revert UnexpectedSender(expectedSender, bodySender);

        bool ok = messageTransmitter.receiveMessage(message, attestation);
        if (!ok) revert ReceiveMessageFailed();

        uint64 sourceChainId = domainToChainId[sourceDomain];
        messageId = _computeMessageId(sourceDomain, nonce);
        IBankReserveSink(bank_).completeReserveBridge(sourceChainId, amount, messageId);

        emit BridgeInCompleted(messageId, sourceChainId, sourceDomain, nonce, amount);
    }

    // ── Internal ───────────────────────────────────────────────────────

    function _computeMessageId(uint32 sourceDomain, uint64 nonce) private pure returns (bytes32) {
        return keccak256(abi.encode(BRIDGE_TYPE, sourceDomain, nonce));
    }

    function _addressToBytes32(address a) private pure returns (bytes32) {
        return bytes32(uint256(uint160(a)));
    }

    function _readUint32(bytes calldata b, uint256 offset) private pure returns (uint32 v) {
        // CCTP fields are big-endian; load 32 bytes and right-shift to discard padding past the field.
        bytes32 word;
        assembly {
            word := calldataload(add(b.offset, offset))
        }
        v = uint32(uint256(word) >> 224);
    }

    function _readUint64(bytes calldata b, uint256 offset) private pure returns (uint64 v) {
        bytes32 word;
        assembly {
            word := calldataload(add(b.offset, offset))
        }
        v = uint64(uint256(word) >> 192);
    }

    function _readBytes32(bytes calldata b, uint256 offset) private pure returns (bytes32 word) {
        assembly {
            word := calldataload(add(b.offset, offset))
        }
    }

    function _readUint256(bytes calldata b, uint256 offset) private pure returns (uint256 v) {
        bytes32 word;
        assembly {
            word := calldataload(add(b.offset, offset))
        }
        v = uint256(word);
    }
}
