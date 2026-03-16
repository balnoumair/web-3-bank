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

    // ── State ──────────────────────────────────────────────────────────

    /// @notice Contract owner (deployer). Can manage the publisher allowlist.
    address public owner;

    /// @notice Authorized publisher addresses.
    mapping(address => bool) public publishers;

    /// @notice Latest route per customer.
    mapping(string customerId => RouteRecord) private _latestRoutes;

    /// @notice Replay guard — true if a runId has already been published.
    mapping(string runId => bool) private _publishedRuns;

    // ── Events ─────────────────────────────────────────────────────────

    /// @dev The indexed `runIdHash` topic is keccak256(bytes(runId)) per Solidity's
    ///      indexed-string behavior. Use the unindexed `runId` param to read the value.
    event RoutePublished(
        string indexed runIdHash,
        string runId,
        string customerId,
        string recommendedChain,
        uint256 score,
        uint256 timestamp
    );

    event ActivationPublished(
        string indexed runIdHash,
        string runId,
        string customerId,
        uint256 thresholdBps,
        string activeChainsCsv,
        string inactiveChainsCsv,
        uint256 timestamp
    );

    event PublisherAdded(address indexed publisher);
    event PublisherRemoved(address indexed publisher);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    // ── Modifiers ──────────────────────────────────────────────────────

    modifier onlyOwner() {
        require(msg.sender == owner, "RouteReceiver: not owner");
        _;
    }

    modifier onlyPublisher() {
        require(publishers[msg.sender], "RouteReceiver: not authorized publisher");
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

    /// @notice Transfer contract ownership to a new address.
    /// @dev Pre-mainnet: consider a two-step transfer pattern for safety.
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "RouteReceiver: zero address");
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }

    // ── Core write ─────────────────────────────────────────────────────

    /// @notice Publish a route scoring result. Reverts on duplicate runId.
    /// @param runId       Canonical run identifier (idempotency key).
    /// @param customerId  Customer this route belongs to.
    /// @param recommendedChain  Winning chain after scoring.
    /// @param score       Scaled integer score.
    /// @param timestamp   Unix epoch seconds of the scoring run.
    function publishRoute(
        string calldata runId,
        string calldata customerId,
        string calldata recommendedChain,
        uint256 score,
        uint256 timestamp
    ) external onlyPublisher {
        require(!_publishedRuns[runId], "RouteReceiver: runId already published");

        _publishedRuns[runId] = true;
        _latestRoutes[customerId] = RouteRecord({
            runId: runId,
            recommendedChain: recommendedChain,
            score: score,
            timestamp: timestamp
        });

        emit RoutePublished(
            runId,
            runId,
            customerId,
            recommendedChain,
            score,
            timestamp
        );
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
    ) external onlyPublisher {
        require(!_publishedRuns[runId], "RouteReceiver: runId already published");

        _publishedRuns[runId] = true;

        emit ActivationPublished(
            runId,
            runId,
            customerId,
            thresholdBps,
            activeChainsCsv,
            inactiveChainsCsv,
            timestamp
        );
    }

    // ── Public reads ───────────────────────────────────────────────────

    /// @notice Get the latest stored route for a customer.
    /// @return runId             Run that produced this route.
    /// @return recommendedChain  Winning chain.
    /// @return score             Scaled integer score.
    /// @return timestamp         Unix epoch seconds.
    function getLatestRoute(
        string calldata customerId
    )
        external
        view
        returns (
            string memory runId,
            string memory recommendedChain,
            uint256 score,
            uint256 timestamp
        )
    {
        RouteRecord storage r = _latestRoutes[customerId];
        return (r.runId, r.recommendedChain, r.score, r.timestamp);
    }

    /// @notice Check whether a runId has already been published.
    function isRunPublished(
        string calldata runId
    ) external view returns (bool) {
        return _publishedRuns[runId];
    }
}
