// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControlUpgradeable} from "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {IBurnMintERC20} from "./interfaces/IBurnMintERC20.sol";

/// @title BankContract
/// @notice Handles USDC deposits (minting SyncUSD 1:1) and the hot-path liquidity release
///         on the destination chain.
/// @dev UUPS upgradeable. Role layout:
///      - ADMIN_ROLE:   Timelock — may authorize upgrades and withdraw pool funds
///      - RELAYER_ROLE: Off-chain relayer — may execute hot-path releases and replenish pool
///      - PAUSER_ROLE:  Emergency multisig/watcher — may pause/unpause all mutations
///      - DEFAULT_ADMIN_ROLE: Timelock — may grant/revoke roles
///
/// Hot-path lifecycle:
///   Source chain  → user calls deposit(amount)   → SyncUSD minted 1:1
///   Destination   → relayer calls hotPathRelease  → USDC released from pool immediately
///   Settlement    → relayer calls replenishPool   → marks transfer settled, pool tracked
contract BankContract is
    Initializable,
    AccessControlUpgradeable,
    PausableUpgradeable,
    UUPSUpgradeable
{
    using SafeERC20 for IERC20;

    // ── Errors ─────────────────────────────────────────────────────────

    error ZeroAddress();
    error ZeroAmount();
    error InsufficientPool(uint256 requested, uint256 available);
    error TransferAlreadyReleased(bytes32 transferId);
    error TransferAlreadySettled(bytes32 transferId);

    // ── Roles ──────────────────────────────────────────────────────────

    bytes32 public constant ADMIN_ROLE   = keccak256("ADMIN_ROLE");
    bytes32 public constant RELAYER_ROLE = keccak256("RELAYER_ROLE");
    bytes32 public constant PAUSER_ROLE  = keccak256("PAUSER_ROLE");

    // ── State ──────────────────────────────────────────────────────────

    /// @notice USDC token accepted for deposits and released via the hot path.
    IERC20 public usdc;

    /// @notice SyncUSD token minted on deposit, burned on settlement.
    IBurnMintERC20 public syncUsd;

    /// @notice USDC balance reserved for hot-path releases.
    uint256 public hotPathPool;

    /// @notice transferId → true once hotPathRelease has been executed.
    mapping(bytes32 => bool) public released;

    /// @notice transferId → true once replenishPool has been called for it.
    mapping(bytes32 => bool) public settled;

    // ── Events ─────────────────────────────────────────────────────────

    event Deposited(address indexed user, uint256 usdcAmount);
    event HotPathReleased(bytes32 indexed transferId, address indexed recipient, uint256 amount);
    event PoolFunded(address indexed funder, uint256 amount);
    event PoolReplenished(bytes32 indexed transferId, uint256 amount);
    event PoolWithdrawn(address indexed to, uint256 amount);

    // ── Constructor ────────────────────────────────────────────────────

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    // ── Initializer ────────────────────────────────────────────────────

    /// @notice Initializes the proxy. Called once by the deployer.
    /// @param admin   Receives DEFAULT_ADMIN_ROLE and ADMIN_ROLE (Timelock in production).
    /// @param pauser  Receives PAUSER_ROLE (emergency multisig in production).
    /// @param usdc_    Address of the USDC token on this chain.
    /// @param syncUsd_ Address of the deployed SyncUSD proxy.
    function initialize(
        address admin,
        address pauser,
        address usdc_,
        address syncUsd_
    ) external initializer {
        if (admin == address(0) || pauser == address(0)) revert ZeroAddress();
        if (usdc_ == address(0) || syncUsd_ == address(0)) revert ZeroAddress();

        __AccessControl_init();
        __Pausable_init();

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, pauser);

        usdc    = IERC20(usdc_);
        syncUsd = IBurnMintERC20(syncUsd_);
    }

    // ── Source-chain: Deposit ──────────────────────────────────────────

    /// @notice Deposit `amount` USDC; receive an equal amount of SyncUSD.
    /// @dev Caller must have approved this contract for `amount` USDC.
    ///      BankContract must hold MINTER_ROLE on SyncUSD.
    function deposit(uint256 amount) external whenNotPaused {
        if (amount == 0) revert ZeroAmount();

        usdc.safeTransferFrom(msg.sender, address(this), amount);
        syncUsd.mint(msg.sender, amount);

        emit Deposited(msg.sender, amount);
    }

    // ── Destination-chain: Hot-path release ───────────────────────────

    /// @notice Fund the hot-path pool so the relayer can serve instant releases.
    /// @dev Anyone may add liquidity; typical caller is the protocol treasury.
    function fundPool(uint256 amount) external whenNotPaused {
        if (amount == 0) revert ZeroAmount();

        usdc.safeTransferFrom(msg.sender, address(this), amount);
        hotPathPool += amount;

        emit PoolFunded(msg.sender, amount);
    }

    /// @notice Immediately release `amount` USDC to `recipient` from the pool.
    /// @dev Only RELAYER_ROLE. Reverts if pool has insufficient balance.
    ///      Each transferId may only be released once.
    function hotPathRelease(
        bytes32 transferId,
        address recipient,
        uint256 amount
    ) external onlyRole(RELAYER_ROLE) whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        if (recipient == address(0)) revert ZeroAddress();
        if (released[transferId]) revert TransferAlreadyReleased(transferId);
        if (hotPathPool < amount) revert InsufficientPool(amount, hotPathPool);

        released[transferId] = true;
        hotPathPool -= amount;
        usdc.safeTransfer(recipient, amount);

        emit HotPathReleased(transferId, recipient, amount);
    }

    /// @notice Mark a transfer as settled and account for pool replenishment.
    /// @dev Called by the relayer when the cross-chain message has been processed.
    ///      In production the USDC would arrive via CCIP; here the relayer signals
    ///      settlement so the accounting stays consistent.
    function replenishPool(bytes32 transferId, uint256 amount) external onlyRole(RELAYER_ROLE) whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        if (settled[transferId]) revert TransferAlreadySettled(transferId);

        settled[transferId] = true;
        hotPathPool += amount;

        emit PoolReplenished(transferId, amount);
    }

    // ── Admin ──────────────────────────────────────────────────────────

    /// @notice Withdraw USDC from the pool. Only ADMIN_ROLE.
    function withdrawPool(address to, uint256 amount) external onlyRole(ADMIN_ROLE) {
        if (to == address(0)) revert ZeroAddress();
        if (amount == 0) revert ZeroAmount();
        if (hotPathPool < amount) revert InsufficientPool(amount, hotPathPool);

        hotPathPool -= amount;
        usdc.safeTransfer(to, amount);

        emit PoolWithdrawn(to, amount);
    }

    // ── Pause ──────────────────────────────────────────────────────────

    /// @notice Pause all deposits, pool operations, and hot-path releases.
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    /// @notice Unpause the contract.
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
