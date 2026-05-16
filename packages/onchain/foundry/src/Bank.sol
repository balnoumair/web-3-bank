// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {AccessControlUpgradeable} from "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC20Metadata} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {IBurnMintERC20} from "./interfaces/IBurnMintERC20.sol";
import {IReserveBridge} from "./interfaces/IReserveBridge.sol";

/// @title Bank
/// @notice Liquidity pool contract managing deposits, withdrawals, and hot path cross-chain transfers.
/// @dev UUPS upgradeable. One instance deployed per chain.
///      - RELAYER_ROLE: Treasury Service relayer — may call `releaseHotPath`
///      - REBALANCER_ROLE: Treasury Service cold-path signer — may call `rebalance`
///      - ADMIN_ROLE:   Protected by Timelock — may authorize upgrades and set fee collector
///      - PAUSER_ROLE:  Emergency multisig — may pause/unpause
///      - DEFAULT_ADMIN_ROLE: Timelock — may grant/revoke roles
///
///      The Bank must be granted MINTER_ROLE on SyncUSD post-deploy to enable deposit/withdraw.
contract Bank is Initializable, AccessControlUpgradeable, PausableUpgradeable, ReentrancyGuard, UUPSUpgradeable {
    using SafeERC20 for IERC20;

    // ── Errors ─────────────────────────────────────────────────────────

    error ZeroAddress();
    error ZeroAmount();
    error InsufficientPoolLiquidity();
    error TransferAlreadyReleased(bytes32 sourceEventHash);
    error TokenNotAllowed(address token);
    error InvalidTokenDecimals(address token, uint8 decimals);
    error RebalanceCapExceeded(uint256 amount, uint256 maxRebalanceAmount);
    error DestChainNotAllowlisted(uint64 destChainId);
    error SourceContractNotAllowlisted(uint64 sourceChainId, address sourceContract);
    error RebalanceMessageAlreadyProcessed(bytes32 messageId);
    error UnauthorizedCcipRouter(address caller);
    error ReserveBridgeNotSet();
    error ReserveTokenNotSet();
    error ReserveRebalanceCapExceeded(uint256 amount, uint256 maxReserveRebalanceAmount);
    error InsufficientReserveLiquidity();
    error ReserveBridgeMessageAlreadyProcessed(bytes32 messageId);
    error UnauthorizedReserveBridge(address caller);
    error ContractFrozenForDecommission();
    error ContractPermanentlyPaused();

    // ── Roles ──────────────────────────────────────────────────────────

    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant RELAYER_ROLE = keccak256("RELAYER_ROLE");
    bytes32 public constant REBALANCER_ROLE = keccak256("REBALANCER_ROLE");
    bytes32 public constant RESERVE_REBALANCER_ROLE = keccak256("RESERVE_REBALANCER_ROLE");
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

    /// @notice Emitted when a token is added to the deposit/withdrawal allowlist.
    event TokenAllowed(address indexed token);

    /// @notice Emitted when a token is removed from the deposit/withdrawal allowlist.
    event TokenDisallowed(address indexed token);

    /// @notice Emitted when cold-path pool liquidity is burned for cross-chain rebalance.
    event RebalanceInitiated(bytes32 indexed messageId, uint64 indexed destChainId, uint256 amount);

    /// @notice Emitted when cold-path pool liquidity is minted after cross-chain delivery.
    event RebalanceCompleted(bytes32 indexed messageId, uint64 indexed sourceChainId, uint256 amount);

    /// @notice Emitted when the maximum permitted single rebalance amount is updated.
    event MaxRebalanceAmountUpdated(uint256 amount);

    /// @notice Emitted when an outbound CCIP destination chain is allowlisted or removed.
    event AllowlistedDestChainUpdated(uint64 indexed destChainId, bool allowed);

    /// @notice Emitted when an inbound CCIP source Bank contract is allowlisted or removed.
    event AllowlistedSourceContractUpdated(uint64 indexed sourceChainId, address indexed sourceContract, bool allowed);

    /// @notice Emitted when the CCIP router allowed to deliver inbound messages is updated.
    event CcipRouterUpdated(address indexed router);

    /// @notice Emitted when the reserve token is updated.
    event ReserveTokenUpdated(address indexed token);

    /// @notice Emitted when the reserve bridge adapter is updated.
    event ReserveBridgeUpdated(address indexed reserveBridge);

    /// @notice Emitted when the maximum permitted single reserve rebalance amount is updated.
    event MaxReserveRebalanceAmountUpdated(uint256 amount);

    /// @notice Emitted when an outbound reserve destination is updated.
    event ReserveDestinationUpdated(uint64 indexed destChainId, address indexed destReserve);

    /// @notice Emitted when USDC reserve bridging is initiated.
    event ReserveBridgeInitiated(
        bytes32 indexed messageId, uint64 indexed destChainId, uint256 amount, bytes32 indexed bridgeType
    );

    /// @notice Emitted when USDC reserve bridging is completed.
    event ReserveBridgeCompleted(bytes32 indexed messageId, uint64 indexed sourceChainId, uint256 amount);
    event FrozenForDecommission(address indexed account);
    event PermanentlyPaused(address indexed account);

    // ── State ──────────────────────────────────────────────────────────

    /// @notice The SyncUSD token this Bank mints/burns.
    IBurnMintERC20 public syncUSD;

    /// @notice Address that receives protocol fees (configurable by ADMIN_ROLE; reserved for future use).
    address public feeCollector;

    /// @dev Monotonic counter used to ensure unique event hashes across transfers.
    uint256 private _nonce;

    /// @notice sourceEventHash → true once releaseHotPath has been executed (idempotency guard).
    mapping(bytes32 => bool) public released;

    /// @notice Tokens approved for deposit and withdrawal.
    mapping(address => bool) public allowedTokens;

    /// @notice Maximum SyncUSD amount that may be rebalanced in one cold-path operation.
    uint256 public maxRebalanceAmount;

    /// @notice CCIP router authorized to deliver inbound rebalance messages.
    address public ccipRouter;

    /// @notice Destination CCIP chain selectors permitted for outbound rebalances.
    mapping(uint64 => bool) public allowlistedDestChains;

    /// @notice Source chain + Bank contract pairs permitted for inbound rebalances.
    mapping(uint64 => mapping(address => bool)) public allowlistedSourceContracts;

    /// @notice CCIP message IDs already processed by this Bank.
    mapping(bytes32 => bool) public processedMessages;

    /// @dev Monotonic counter for deterministic cold-path message IDs.
    uint256 private _rebalanceNonce;

    /// @notice Underlying reserve token bridged by reserve rebalance operations.
    address public reserveToken;

    /// @notice Bridge adapter used for outbound and inbound reserve bridge operations.
    IReserveBridge public reserveBridge;

    /// @notice Maximum USDC amount that may be bridged in one reserve rebalance operation.
    uint256 public maxReserveRebalanceAmount;

    /// @notice Reserve bridge message IDs already processed by this Bank.
    mapping(bytes32 => bool) public processedReserveMessages;

    /// @notice Destination Bank/reserve address per chain for outbound reserve bridge operations.
    mapping(uint64 => address) public reserveDestinations;

    /// @notice One-way decommission freeze. Blocks new user/hot-path operations but leaves drain functions live.
    bool public frozen;

    /// @notice Permanent pause flag. Once set, `unpause` is disabled.
    bool public permanentPause;

    modifier whenNotFrozenForDecommission() {
        if (frozen) revert ContractFrozenForDecommission();
        _;
    }

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
    function initialize(address admin, address pauser, address syncUSD_, address feeCollector_) external initializer {
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
    ///      Token must be on the allowlist and have exactly 6 decimals.
    /// @param underlyingToken The accepted collateral token (e.g. USDC).
    /// @param amount          Amount of underlying token to deposit.
    function deposit(address underlyingToken, uint256 amount)
        external
        whenNotPaused
        whenNotFrozenForDecommission
        nonReentrant
    {
        if (!allowedTokens[underlyingToken]) revert TokenNotAllowed(underlyingToken);
        if (amount == 0) revert ZeroAmount();
        uint8 decimals = IERC20Metadata(underlyingToken).decimals();
        if (decimals != 6) revert InvalidTokenDecimals(underlyingToken, decimals);
        IERC20(underlyingToken).safeTransferFrom(msg.sender, address(this), amount);
        syncUSD.mint(msg.sender, amount);
        emit Deposited(msg.sender, underlyingToken, amount);
    }

    /// @notice Burns the caller's SyncUSD and releases equivalent `underlyingToken`.
    /// @dev Caller must approve this contract to spend `amount` of SyncUSD.
    ///      Token must be on the allowlist and have exactly 6 decimals.
    /// @param underlyingToken The underlying token to release (e.g. USDC).
    /// @param amount          Amount of SyncUSD to burn / underlying to receive.
    function withdraw(address underlyingToken, uint256 amount) external whenNotPaused nonReentrant {
        if (!allowedTokens[underlyingToken]) revert TokenNotAllowed(underlyingToken);
        if (amount == 0) revert ZeroAmount();
        uint8 decimals = IERC20Metadata(underlyingToken).decimals();
        if (decimals != 6) revert InvalidTokenDecimals(underlyingToken, decimals);
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
        whenNotFrozenForDecommission
        nonReentrant
    {
        if (to == address(0)) revert ZeroAddress();
        if (amount == 0) revert ZeroAmount();

        IERC20(address(syncUSD)).safeTransferFrom(msg.sender, address(this), amount);

        uint256 fee = 0; // Reserved
        bytes32 eventHash = keccak256(abi.encode(msg.sender, to, amount, destinationChainId, block.chainid, _nonce++));

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
        whenNotFrozenForDecommission
        nonReentrant
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

    /// @notice Burns local pool SyncUSD and emits a CCIP-compatible cold-path message ID.
    /// @dev The emitted `messageId` is the audit key Treasury records for the rebalance op.
    function rebalance(uint64 destChainId, uint256 amount)
        external
        whenNotPaused
        nonReentrant
        onlyRole(REBALANCER_ROLE)
        returns (bytes32 messageId)
    {
        if (amount == 0) revert ZeroAmount();
        if (amount > maxRebalanceAmount) revert RebalanceCapExceeded(amount, maxRebalanceAmount);
        if (!allowlistedDestChains[destChainId]) revert DestChainNotAllowlisted(destChainId);
        if (IERC20(address(syncUSD)).balanceOf(address(this)) < amount) {
            revert InsufficientPoolLiquidity();
        }

        messageId = keccak256(abi.encode(block.chainid, address(this), destChainId, amount, _rebalanceNonce++));
        syncUSD.burn(amount);
        emit RebalanceInitiated(messageId, destChainId, amount);
    }

    /// @notice Bridges USDC reserve liquidity to a destination chain Bank.
    /// @dev The registered adapter is expected to pull `amount` USDC using the approval set here.
    function bridgeReserve(uint64 destChainId, uint256 amount)
        external
        whenNotPaused
        nonReentrant
        onlyRole(RESERVE_REBALANCER_ROLE)
        returns (bytes32 messageId)
    {
        IReserveBridge bridge = reserveBridge;
        if (address(bridge) == address(0)) revert ReserveBridgeNotSet();
        address token = reserveToken;
        if (token == address(0)) revert ReserveTokenNotSet();
        if (amount == 0) revert ZeroAmount();
        if (amount > maxReserveRebalanceAmount) {
            revert ReserveRebalanceCapExceeded(amount, maxReserveRebalanceAmount);
        }
        if (!allowlistedDestChains[destChainId]) revert DestChainNotAllowlisted(destChainId);
        address destReserve = reserveDestinations[destChainId];
        if (destReserve == address(0)) revert DestChainNotAllowlisted(destChainId);
        if (IERC20(token).balanceOf(address(this)) < amount) revert InsufficientReserveLiquidity();

        IERC20(token).forceApprove(address(bridge), amount);
        messageId = bridge.bridgeOut(destChainId, amount, destReserve);
        IERC20(token).forceApprove(address(bridge), 0);

        emit ReserveBridgeInitiated(messageId, destChainId, amount, bridge.bridgeType());
    }

    /// @notice Receives a cold-path rebalance delivery from the CCIP execution surface.
    /// @dev Source chain and source Bank are checked against the inbound allowlist.
    function ccipReceive(uint64 sourceChainId, address sourceContract, uint256 amount, bytes32 messageId)
        external
        whenNotPaused
        nonReentrant
    {
        if (msg.sender != ccipRouter) revert UnauthorizedCcipRouter(msg.sender);
        _ccipReceive(sourceChainId, sourceContract, amount, messageId);
    }

    function _ccipReceive(uint64 sourceChainId, address sourceContract, uint256 amount, bytes32 messageId) internal {
        if (sourceContract == address(0)) revert ZeroAddress();
        if (amount == 0) revert ZeroAmount();
        if (!allowlistedSourceContracts[sourceChainId][sourceContract]) {
            revert SourceContractNotAllowlisted(sourceChainId, sourceContract);
        }
        if (processedMessages[messageId]) revert RebalanceMessageAlreadyProcessed(messageId);

        processedMessages[messageId] = true;
        syncUSD.mint(address(this), amount);
        emit RebalanceCompleted(messageId, sourceChainId, amount);
    }

    /// @notice Records an inbound reserve bridge delivery from the registered adapter.
    /// @dev The adapter releases/mints USDC before calling this function; Bank records idempotency and audit.
    function completeReserveBridge(uint64 sourceChainId, uint256 amount, bytes32 messageId)
        external
        whenNotPaused
        nonReentrant
    {
        if (msg.sender != address(reserveBridge)) revert UnauthorizedReserveBridge(msg.sender);
        if (amount == 0) revert ZeroAmount();
        if (processedReserveMessages[messageId]) revert ReserveBridgeMessageAlreadyProcessed(messageId);

        processedReserveMessages[messageId] = true;
        emit ReserveBridgeCompleted(messageId, sourceChainId, amount);
    }

    /// @notice Returns the SyncUSD balance held in this pool, available for hot-path releases.
    function poolDepth() external view returns (uint256) {
        return IERC20(address(syncUSD)).balanceOf(address(this));
    }

    /// @notice Returns the USDC reserve balance held by this Bank.
    function reserveDepth() external view returns (uint256) {
        address token = reserveToken;
        if (token == address(0)) return 0;
        return IERC20(token).balanceOf(address(this));
    }

    // ── Admin ──────────────────────────────────────────────────────────

    /// @notice Updates the fee collector address. Restricted to ADMIN_ROLE.
    function setFeeCollector(address feeCollector_) external onlyRole(ADMIN_ROLE) {
        feeCollector = feeCollector_;
        emit FeeCollectorUpdated(feeCollector_);
    }

    /// @notice Adds a token to the deposit/withdrawal allowlist. Restricted to ADMIN_ROLE.
    function allowToken(address token) external onlyRole(ADMIN_ROLE) {
        if (token == address(0)) revert ZeroAddress();
        allowedTokens[token] = true;
        emit TokenAllowed(token);
        if (reserveToken == address(0)) {
            reserveToken = token;
            emit ReserveTokenUpdated(token);
        }
    }

    /// @notice Removes a token from the deposit/withdrawal allowlist. Restricted to ADMIN_ROLE.
    function disallowToken(address token) external onlyRole(ADMIN_ROLE) {
        allowedTokens[token] = false;
        emit TokenDisallowed(token);
    }

    /// @notice Updates the underlying reserve token. Restricted to ADMIN_ROLE.
    function setReserveToken(address token) external onlyRole(ADMIN_ROLE) {
        if (token == address(0)) revert ZeroAddress();
        if (!allowedTokens[token]) revert TokenNotAllowed(token);
        reserveToken = token;
        emit ReserveTokenUpdated(token);
    }

    /// @notice Updates the per-call cold-path rebalance cap. Restricted to ADMIN_ROLE.
    function setMaxRebalanceAmount(uint256 amount) external onlyRole(ADMIN_ROLE) {
        maxRebalanceAmount = amount;
        emit MaxRebalanceAmountUpdated(amount);
    }

    /// @notice Updates the per-call reserve rebalance cap. Restricted to ADMIN_ROLE.
    function setMaxReserveRebalanceAmount(uint256 amount) external onlyRole(ADMIN_ROLE) {
        maxReserveRebalanceAmount = amount;
        emit MaxReserveRebalanceAmountUpdated(amount);
    }

    /// @notice Updates the reserve bridge adapter. Restricted to ADMIN_ROLE.
    function setReserveBridge(IReserveBridge bridge) external onlyRole(ADMIN_ROLE) {
        if (address(bridge) == address(0)) revert ZeroAddress();
        reserveBridge = bridge;
        emit ReserveBridgeUpdated(address(bridge));
    }

    /// @notice Updates the destination Bank/reserve address for reserve bridge operations.
    function setReserveDestination(uint64 destChainId, address destReserve) external onlyRole(ADMIN_ROLE) {
        reserveDestinations[destChainId] = destReserve;
        emit ReserveDestinationUpdated(destChainId, destReserve);
    }

    /// @notice Updates the CCIP router authorized to call `ccipReceive`. Restricted to ADMIN_ROLE.
    function setCcipRouter(address router) external onlyRole(ADMIN_ROLE) {
        if (router == address(0)) revert ZeroAddress();
        ccipRouter = router;
        emit CcipRouterUpdated(router);
    }

    /// @notice Adds or removes an outbound cold-path destination. Restricted to ADMIN_ROLE.
    function setAllowlistedDestChain(uint64 destChainId, bool allowed) external onlyRole(ADMIN_ROLE) {
        allowlistedDestChains[destChainId] = allowed;
        emit AllowlistedDestChainUpdated(destChainId, allowed);
    }

    /// @notice Adds or removes an inbound cold-path source Bank. Restricted to ADMIN_ROLE.
    function setAllowlistedSourceContract(uint64 sourceChainId, address sourceContract, bool allowed)
        external
        onlyRole(ADMIN_ROLE)
    {
        if (sourceContract == address(0)) revert ZeroAddress();
        allowlistedSourceContracts[sourceChainId][sourceContract] = allowed;
        emit AllowlistedSourceContractUpdated(sourceChainId, sourceContract, allowed);
    }

    // ── Pause ──────────────────────────────────────────────────────────

    /// @notice Pauses all state-mutating functions.
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    /// @notice Unpauses the contract.
    function unpause() external onlyRole(PAUSER_ROLE) {
        if (permanentPause) revert ContractPermanentlyPaused();
        _unpause();
    }

    /// @notice One-shot governance freeze for chain decommissioning.
    function freezeForDecommission() external onlyRole(ADMIN_ROLE) {
        if (frozen) revert ContractFrozenForDecommission();
        frozen = true;
        emit FrozenForDecommission(msg.sender);
    }

    /// @notice Permanently pause all state-changing Bank operations after drain completion.
    function pausePermanently() external onlyRole(ADMIN_ROLE) {
        if (permanentPause) revert ContractPermanentlyPaused();
        permanentPause = true;
        _pause();
        emit PermanentlyPaused(msg.sender);
    }

    // ── UUPS ───────────────────────────────────────────────────────────

    /// @dev Only ADMIN_ROLE may authorize upgrades.
    function _authorizeUpgrade(address newImplementation) internal override onlyRole(ADMIN_ROLE) {}
}
