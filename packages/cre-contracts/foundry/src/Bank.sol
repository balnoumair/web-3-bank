// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControlUpgradeable} from "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {IBurnMintERC20} from "./interfaces/IBurnMintERC20.sol";

/// @title Bank
/// @notice Liquidity pool contract managing deposits, withdrawals, and hot path cross-chain transfers.
/// @dev UUPS upgradeable. One instance deployed per chain.
///      - RELAYER_ROLE: Treasury Service relayer — may call `releaseHotPath`
///      - ADMIN_ROLE:   Protected by Timelock — may authorize upgrades and set fee collector
///      - PAUSER_ROLE:  Emergency multisig — may pause/unpause
///      - DEFAULT_ADMIN_ROLE: Timelock — may grant/revoke roles
///
///      The Bank must be granted MINTER_ROLE on SyncUSD post-deploy to enable deposit/withdraw.
contract Bank is
    Initializable,
    AccessControlUpgradeable,
    PausableUpgradeable,
    UUPSUpgradeable
{
    using SafeERC20 for IERC20;

    // ── Errors ─────────────────────────────────────────────────────────

    error ZeroAddress();
    error ZeroAmount();
    error InsufficientPoolLiquidity();
    error TransferAlreadyReleased(bytes32 sourceEventHash);

    // ── Roles ──────────────────────────────────────────────────────────

    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant RELAYER_ROLE = keccak256("RELAYER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    // ── Events ─────────────────────────────────────────────────────────

    /// @notice Emitted when a hot path transfer is initiated on the source chain.
    /// @param eventHash Unique identifier for this transfer — used as `sourceEventHash` on the destination chain.
    /// @param fee       Reserved fee amount (currently always 0).
    event HotPathInitiated(
        address indexed sender,
        address indexed to,
        uint256 amount,
        uint256 destinationChainId,
        bytes32 eventHash,
        uint256 fee
    );

    /// @notice Emitted when hot path liquidity is released on the destination chain.
    event HotPathReleased(address indexed to, uint256 amount, bytes32 indexed sourceEventHash);

    /// @notice Emitted when a user deposits an underlying token and receives SyncUSD.
    event Deposited(address indexed user, address underlyingToken, uint256 amount);

    /// @notice Emitted when a user withdraws an underlying token by burning SyncUSD.
    event Withdrawn(address indexed user, address underlyingToken, uint256 amount);

    /// @notice Emitted when the fee collector address is updated.
    event FeeCollectorUpdated(address indexed feeCollector);

    // ── State ──────────────────────────────────────────────────────────

    /// @notice The SyncUSD token this Bank mints/burns.
    IBurnMintERC20 public syncUSD;

    /// @notice Address that receives protocol fees (configurable by ADMIN_ROLE; reserved for future use).
    address public feeCollector;

    /// @dev Monotonic counter used to ensure unique event hashes across transfers.
    uint256 private _nonce;

    /// @notice sourceEventHash → true once releaseHotPath has been executed (idempotency guard).
    mapping(bytes32 => bool) public released;

    // ── Constructor ────────────────────────────────────────────────────

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    // ── Initializer ────────────────────────────────────────────────────

    /// @notice Initializes the proxy. Called once by the deployer.
    /// @param admin        Receives DEFAULT_ADMIN_ROLE and ADMIN_ROLE (Timelock in production).
    /// @param pauser       Receives PAUSER_ROLE (emergency multisig in production).
    /// @param syncUSD_     Address of the SyncUSD token proxy.
    /// @param feeCollector_ Initial fee collector address (may be address(0) to disable fees).
    function initialize(
        address admin,
        address pauser,
        address syncUSD_,
        address feeCollector_
    ) external initializer {
        if (admin == address(0) || pauser == address(0) || syncUSD_ == address(0)) revert ZeroAddress();

        __AccessControl_init();
        __Pausable_init();

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, pauser);

        syncUSD = IBurnMintERC20(syncUSD_);
        feeCollector = feeCollector_;
    }

    // ── Core Methods ───────────────────────────────────────────────────

    /// @notice Escrows `underlyingToken` and mints equivalent SyncUSD to the caller.
    /// @dev Caller must approve this contract to spend `amount` of `underlyingToken`.
    ///      Fee is reserved and currently set to 0.
    /// @param underlyingToken The accepted collateral token (e.g. USDC).
    /// @param amount          Amount of underlying token to deposit.
    function deposit(address underlyingToken, uint256 amount) external whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        IERC20(underlyingToken).safeTransferFrom(msg.sender, address(this), amount);
        syncUSD.mint(msg.sender, amount);
        emit Deposited(msg.sender, underlyingToken, amount);
    }

    /// @notice Burns the caller's SyncUSD and releases equivalent `underlyingToken`.
    /// @dev Caller must approve this contract to spend `amount` of SyncUSD.
    ///      Fee is reserved and currently set to 0.
    /// @param underlyingToken The underlying token to release (e.g. USDC).
    /// @param amount          Amount of SyncUSD to burn / underlying to receive.
    function withdraw(address underlyingToken, uint256 amount) external whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        syncUSD.burnFrom(msg.sender, amount);
        IERC20(underlyingToken).safeTransfer(msg.sender, amount);
        emit Withdrawn(msg.sender, underlyingToken, amount);
    }

    /// @notice Locks the caller's SyncUSD in this pool and emits a cross-chain transfer event.
    /// @dev The off-chain relayer watches for `HotPathInitiated` and calls `releaseHotPath`
    ///      on the destination chain Bank. Caller must approve this contract to spend `amount` of SyncUSD.
    /// @param to                 Recipient address on the destination chain.
    /// @param amount             Amount of SyncUSD to lock and transfer.
    /// @param destinationChainId EIP-155 chain ID of the destination chain.
    function transferHotPath(address to, uint256 amount, uint256 destinationChainId)
        external
        whenNotPaused
    {
        if (to == address(0)) revert ZeroAddress();
        if (amount == 0) revert ZeroAmount();

        IERC20(address(syncUSD)).safeTransferFrom(msg.sender, address(this), amount);

        uint256 fee = 0; // Reserved
        bytes32 eventHash = keccak256(
            abi.encode(msg.sender, to, amount, destinationChainId, block.chainid, _nonce++)
        );

        emit HotPathInitiated(msg.sender, to, amount, destinationChainId, eventHash, fee);
    }

    /// @notice Releases SyncUSD from this pool's liquidity to `to`. Restricted to RELAYER_ROLE.
    /// @dev Reverts if this contract holds insufficient SyncUSD balance.
    /// @param to              Recipient address on this chain.
    /// @param amount          Amount of SyncUSD to release.
    /// @param sourceEventHash The `eventHash` from the source chain's `HotPathInitiated` event.
    function releaseHotPath(address to, uint256 amount, bytes32 sourceEventHash)
        external
        whenNotPaused
        onlyRole(RELAYER_ROLE)
    {
        if (to == address(0)) revert ZeroAddress();
        if (amount == 0) revert ZeroAmount();
        if (released[sourceEventHash]) revert TransferAlreadyReleased(sourceEventHash);
        if (IERC20(address(syncUSD)).balanceOf(address(this)) < amount) {
            revert InsufficientPoolLiquidity();
        }

        released[sourceEventHash] = true;
        IERC20(address(syncUSD)).safeTransfer(to, amount);
        emit HotPathReleased(to, amount, sourceEventHash);
    }

    /// @notice Returns the SyncUSD balance held in this pool, available for hot-path releases.
    function poolDepth() external view returns (uint256) {
        return IERC20(address(syncUSD)).balanceOf(address(this));
    }

    // ── Admin ──────────────────────────────────────────────────────────

    /// @notice Updates the fee collector address. Restricted to ADMIN_ROLE.
    function setFeeCollector(address feeCollector_) external onlyRole(ADMIN_ROLE) {
        feeCollector = feeCollector_;
        emit FeeCollectorUpdated(feeCollector_);
    }

    // ── Pause ──────────────────────────────────────────────────────────

    /// @notice Pauses all state-mutating functions.
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
}
