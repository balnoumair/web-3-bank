// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title RouteReceiver
/// @notice Stores CRE route scoring results onchain with replay protection.
/// @dev Deployed on Base Sepolia as the destination-side receiver for the
///      CRE Route Orchestrator pipeline (ADR-005).
contract RouteReceiver {
    // ── Types ──────────────────────────────────────────────────────────

    struct RouteRecord {
        string runId;
        string recommendedChain;
        uint256 score;
        uint256 timestamp;
    }

    struct ChainDecommissionStatus {
        bool draining;
        bool decommissioned;
    }

    // ── Constants ──────────────────────────────────────────────────────

    uint256 private constant MAX_SHORT_STRING = 128;
    uint256 private constant MAX_CSV_STRING = 512;

    // ── Errors ─────────────────────────────────────────────────────────

    error ContractPaused();
    error StringTooLong();
    error ChainAlreadyDecommissioned(uint256 chainId);
    error ChainNotDraining(uint256 chainId);
    error ActivationTouchesDecommissionedChain(uint256 chainId);

    // ── State ──────────────────────────────────────────────────────────

    /// @notice Contract owner (deployer). Can manage the publisher allowlist.
    address public owner;

    /// @notice Pending owner for two-step ownership transfer.
    address public pendingOwner;

    /// @notice Emergency pause flag. Blocks all write operations when true.
    bool public paused;

    /// @notice Authorized publisher addresses.
    mapping(address => bool) public publishers;

    /// @notice Latest route per customer.
    mapping(string customerId => RouteRecord) private _latestRoutes;

    /// @notice Replay guard for publishRoute — true if a runId has already been route-published.
    mapping(string runId => bool) private _publishedRouteRuns;

    /// @notice Replay guard for publishActivationState — true if a runId has already been activation-published.
    mapping(string runId => bool) private _publishedActivationRuns;

    /// @notice Governance-managed terminal status per EVM chain ID.
    mapping(uint256 chainId => ChainDecommissionStatus) private _chainDecommissionStatus;

    // ── Events ─────────────────────────────────────────────────────────

    /// @dev The indexed `runIdHash` topic is keccak256(bytes(runId)) per Solidity's
    ///      indexed-string behavior. Use the unindexed `runId` param to read the value.
    event RoutePublished(
        string indexed runIdHash,
        string runId,
        string customerId,
        string recommendedChain,
        uint256 score,
        uint256 timestamp,
        uint256 blockTimestamp
    );

    event ActivationPublished(
        string indexed runIdHash,
        string runId,
        string customerId,
        uint256 thresholdBps,
        string activeChainsCsv,
        string inactiveChainsCsv,
        uint256 timestamp,
        uint256 blockTimestamp
    );

    event PublisherAdded(address indexed publisher);
    event PublisherRemoved(address indexed publisher);
    event OwnershipTransferStarted(address indexed previousOwner, address indexed newOwner);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event Paused(address indexed account);
    event Unpaused(address indexed account);
    event ChainDecommissioningMarked(uint256 indexed chainId);
    event ChainDecommissionFinalized(uint256 indexed chainId);

    // ── Modifiers ──────────────────────────────────────────────────────

    modifier onlyOwner() {
        require(msg.sender == owner, "RouteReceiver: not owner");
        _;
    }

    modifier onlyPublisher() {
        require(publishers[msg.sender], "RouteReceiver: not authorized publisher");
        _;
    }

    modifier whenNotPaused() {
        if (paused) revert ContractPaused();
        _;
    }

    // ── Constructor ────────────────────────────────────────────────────

    constructor() {
        owner = msg.sender;
    }

    // ── Publisher management ───────────────────────────────────────────

    /// @notice Add an address to the publisher allowlist.
    function addPublisher(address publisher) external onlyOwner {
        require(publisher != address(0), "RouteReceiver: zero address");
        publishers[publisher] = true;
        emit PublisherAdded(publisher);
    }

    /// @notice Remove an address from the publisher allowlist.
    function removePublisher(address publisher) external onlyOwner {
        publishers[publisher] = false;
        emit PublisherRemoved(publisher);
    }

    // ── Ownership management ──────────────────────────────────────────

    /// @notice Initiates a two-step ownership transfer. The new owner must call `acceptOwnership`.
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "RouteReceiver: zero address");
        pendingOwner = newOwner;
        emit OwnershipTransferStarted(owner, newOwner);
    }

    /// @notice Completes the ownership transfer. Must be called by `pendingOwner`.
    function acceptOwnership() external {
        require(msg.sender == pendingOwner, "RouteReceiver: not pending owner");
        emit OwnershipTransferred(owner, pendingOwner);
        owner = pendingOwner;
        pendingOwner = address(0);
    }

    // ── Emergency pause ───────────────────────────────────────────────

    /// @notice Pauses all write operations. Restricted to owner.
    function pause() external onlyOwner {
        paused = true;
        emit Paused(msg.sender);
    }

    /// @notice Unpauses write operations. Restricted to owner.
    function unpause() external onlyOwner {
        paused = false;
        emit Unpaused(msg.sender);
    }

    // ── Chain decommissioning ─────────────────────────────────────────

    /// @notice Mark a chain as draining before final decommission. Governance only.
    function markDecommissioning(uint256 chainId) external onlyOwner whenNotPaused {
        ChainDecommissionStatus storage status = _chainDecommissionStatus[chainId];
        if (status.decommissioned) revert ChainAlreadyDecommissioned(chainId);
        status.draining = true;
        emit ChainDecommissioningMarked(chainId);
    }

    /// @notice Finalize terminal decommission. Irreversible. Governance only.
    function finalizeDecommission(uint256 chainId) external onlyOwner whenNotPaused {
        ChainDecommissionStatus storage status = _chainDecommissionStatus[chainId];
        if (status.decommissioned) revert ChainAlreadyDecommissioned(chainId);
        if (!status.draining) revert ChainNotDraining(chainId);
        status.decommissioned = true;
        emit ChainDecommissionFinalized(chainId);
    }

    // ── Core write ─────────────────────────────────────────────────────

    /// @notice Publish a route scoring result. Reverts on duplicate runId.
    /// @param runId            Canonical run identifier (idempotency key).
    /// @param customerId       Customer this route belongs to.
    /// @param recommendedChain Winning chain after scoring.
    /// @param score            Scaled integer score.
    /// @param timestamp        Unix epoch seconds of the scoring run.
    function publishRoute(
        string calldata runId,
        string calldata customerId,
        string calldata recommendedChain,
        uint256 score,
        uint256 timestamp
    ) external onlyPublisher whenNotPaused {
        if (bytes(runId).length > MAX_SHORT_STRING) revert StringTooLong();
        if (bytes(customerId).length > MAX_SHORT_STRING) revert StringTooLong();
        if (bytes(recommendedChain).length > MAX_SHORT_STRING) revert StringTooLong();
        require(!_publishedRouteRuns[runId], "RouteReceiver: runId already published");

        _publishedRouteRuns[runId] = true;
        _latestRoutes[customerId] =
            RouteRecord({runId: runId, recommendedChain: recommendedChain, score: score, timestamp: timestamp});

        emit RoutePublished(runId, runId, customerId, recommendedChain, score, timestamp, block.timestamp);
    }

    /// @notice Publish threshold-based activation state evidence.
    /// @dev Stores run replay protection and emits activation payload for cross-network consumers.
    function publishActivationState(
        string calldata runId,
        string calldata customerId,
        uint256 thresholdBps,
        string calldata activeChainsCsv,
        string calldata inactiveChainsCsv,
        uint256 timestamp
    ) external onlyPublisher whenNotPaused {
        if (bytes(runId).length > MAX_SHORT_STRING) revert StringTooLong();
        if (bytes(customerId).length > MAX_SHORT_STRING) revert StringTooLong();
        if (bytes(activeChainsCsv).length > MAX_CSV_STRING) revert StringTooLong();
        if (bytes(inactiveChainsCsv).length > MAX_CSV_STRING) revert StringTooLong();
        require(!_publishedActivationRuns[runId], "RouteReceiver: runId already published");
        _rejectDecommissionedChains(activeChainsCsv);
        _rejectDecommissionedChains(inactiveChainsCsv);

        _publishedActivationRuns[runId] = true;

        emit ActivationPublished(
            runId, runId, customerId, thresholdBps, activeChainsCsv, inactiveChainsCsv, timestamp, block.timestamp
        );
    }

    // ── Public reads ───────────────────────────────────────────────────

    /// @notice Get the latest stored route for a customer.
    /// @return runId             Run that produced this route.
    /// @return recommendedChain  Winning chain.
    /// @return score             Scaled integer score.
    /// @return timestamp         Unix epoch seconds.
    function getLatestRoute(string calldata customerId)
        external
        view
        returns (string memory runId, string memory recommendedChain, uint256 score, uint256 timestamp)
    {
        RouteRecord storage r = _latestRoutes[customerId];
        return (r.runId, r.recommendedChain, r.score, r.timestamp);
    }

    /// @notice Get governance-managed chain decommission status.
    function getChainDecommissionStatus(uint256 chainId) external view returns (bool draining, bool decommissioned) {
        ChainDecommissionStatus storage status = _chainDecommissionStatus[chainId];
        return (status.draining, status.decommissioned);
    }

    /// @notice Latest route plus the terminal status for a consumer-supplied chain ID.
    function getLatestRouteWithChainState(string calldata customerId, uint256 chainId)
        external
        view
        returns (
            string memory runId,
            string memory recommendedChain,
            uint256 score,
            uint256 timestamp,
            bool draining,
            bool decommissioned
        )
    {
        RouteRecord storage r = _latestRoutes[customerId];
        ChainDecommissionStatus storage status = _chainDecommissionStatus[chainId];
        return (r.runId, r.recommendedChain, r.score, r.timestamp, status.draining, status.decommissioned);
    }

    /// @notice Check whether a runId has already been route-published.
    function isRouteRunPublished(string calldata runId) external view returns (bool) {
        return _publishedRouteRuns[runId];
    }

    /// @notice Check whether a runId has already been activation-published.
    function isActivationRunPublished(string calldata runId) external view returns (bool) {
        return _publishedActivationRuns[runId];
    }

    function _rejectDecommissionedChains(string calldata csv) private view {
        bytes calldata data = bytes(csv);
        uint256 current = 0;
        bool inToken = false;
        bool numericToken = false;

        for (uint256 i = 0; i <= data.length; i++) {
            bytes1 ch = i < data.length ? data[i] : bytes1(",");
            bool isDigit = ch >= "0" && ch <= "9";

            if (isDigit) {
                current = current * 10 + (uint8(ch) - uint8(bytes1("0")));
                inToken = true;
                numericToken = true;
                continue;
            }

            if (ch != "," && ch != " ") {
                numericToken = false;
                inToken = true;
                continue;
            }

            if (inToken && numericToken && _chainDecommissionStatus[current].decommissioned) {
                revert ActivationTouchesDecommissionedChain(current);
            }

            current = 0;
            inToken = false;
            numericToken = false;
        }
    }
}
