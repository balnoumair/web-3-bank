import type {
    OnchainPublicationRecord,
    OnchainPublishResult,
    RouteUpdatedOnchain,
} from "../../src";

// ── Onchain payload cases ───────────────────────────────────────────

const validOnchainPayload: RouteUpdatedOnchain = {
    runId: "run-onchain-001",
    customerId: "customer-1",
    recommendedChain: "base-sepolia",
    score: 82,
    timestamp: 1740000000,
};

const minimalOnchainPayload: RouteUpdatedOnchain = {
    runId: "run-onchain-002",
    customerId: "customer-2",
    recommendedChain: "arbitrum-sepolia",
    score: 0,
    timestamp: 0,
};

const activationOnchainPayload: RouteUpdatedOnchain = {
    runId: "run-onchain-activation-001",
    customerId: "customer-activation",
    recommendedChain: "base-sepolia",
    score: 82,
    timestamp: 1740000000,
    thresholdBps: 7000,
    activeChains: ["base-sepolia", "arbitrum-sepolia"],
    inactiveChains: ["optimism-sepolia"],
};

// ── Publication record cases ────────────────────────────────────────

const confirmedPublication: OnchainPublicationRecord = {
    runId: "run-onchain-001",
    customerId: "customer-1",
    txHash: "0xabc123def456789",
    blockNumber: 12345678,
    eventLogIndex: 0,
    publisherAddress: "0x1234567890abcdef1234567890abcdef12345678",
    status: "confirmed",
    errorCode: null,
    errorMessage: null,
    timestamp: "2026-02-21T12:00:00.000Z",
    simulationId: null,
    simulationTraceUrl: null,
};

const confirmedPublicationWithSimulation: OnchainPublicationRecord = {
    runId: "run-onchain-005",
    customerId: "customer-1",
    txHash: "0xabc123def456789",
    blockNumber: 12345678,
    eventLogIndex: 2,
    publisherAddress: "0x1234567890abcdef1234567890abcdef12345678",
    status: "confirmed",
    errorCode: null,
    errorMessage: null,
    timestamp: "2026-02-21T12:04:00.000Z",
    simulationId: "sim-1234567890abcdef",
    simulationTraceUrl:
        "https://dashboard.tenderly.co/account/project/simulator/sim-1234567890abcdef",
};

const failedPublication: OnchainPublicationRecord = {
    runId: "run-onchain-003",
    customerId: "customer-1",
    txHash: null,
    blockNumber: null,
    eventLogIndex: null,
    publisherAddress: "0x1234567890abcdef1234567890abcdef12345678",
    status: "failed",
    errorCode: "PUBLISH_REVERTED",
    errorMessage: "Unauthorized publisher",
    timestamp: "2026-02-21T12:01:00.000Z",
    simulationId: null,
    simulationTraceUrl: null,
};

const duplicatePublication: OnchainPublicationRecord = {
    runId: "run-onchain-001",
    customerId: "customer-1",
    txHash: null,
    blockNumber: null,
    eventLogIndex: null,
    publisherAddress: "0x1234567890abcdef1234567890abcdef12345678",
    status: "duplicateRunId",
    errorCode: null,
    errorMessage: null,
    timestamp: "2026-02-21T12:02:00.000Z",
    simulationId: null,
    simulationTraceUrl: null,
};

const pendingPublication: OnchainPublicationRecord = {
    runId: "run-onchain-004",
    customerId: "customer-2",
    txHash: null,
    blockNumber: null,
    eventLogIndex: null,
    publisherAddress: "0x1234567890abcdef1234567890abcdef12345678",
    status: "pending",
    errorCode: null,
    errorMessage: null,
    timestamp: "2026-02-21T12:03:00.000Z",
    simulationId: null,
    simulationTraceUrl: null,
};

// ── Publish result cases ────────────────────────────────────────────

const confirmedResult: OnchainPublishResult = {
    status: "confirmed",
    txHash: "0xabc123def456789",
    blockNumber: 12345678,
    eventLogIndex: 0,
};

const confirmedResultWithSimulation: OnchainPublishResult = {
    status: "confirmed",
    txHash: "0xabc123def456789",
    blockNumber: 12345678,
    eventLogIndex: 0,
    simulationId: "sim-1234567890abcdef",
    simulationTraceUrl:
        "https://dashboard.tenderly.co/account/project/simulator/sim-1234567890abcdef",
};

const duplicateResult: OnchainPublishResult = {
    status: "duplicateRunId",
    runId: "run-onchain-001",
};

const failedResult: OnchainPublishResult = {
    status: "failed",
    error: "Unauthorized publisher",
};

// ── Exports ─────────────────────────────────────────────────────────

export const ONCHAIN_PAYLOAD_CASES = {
    valid: validOnchainPayload,
    minimal: minimalOnchainPayload,
    activation: activationOnchainPayload,
} as const;

export const ONCHAIN_PUBLICATION_CASES = {
    confirmed: confirmedPublication,
    confirmedWithSimulation: confirmedPublicationWithSimulation,
    failed: failedPublication,
    duplicate: duplicatePublication,
    pending: pendingPublication,
} as const;

export const ONCHAIN_PUBLISH_RESULT_CASES = {
    confirmed: confirmedResult,
    confirmedWithSimulation: confirmedResultWithSimulation,
    duplicate: duplicateResult,
    failed: failedResult,
} as const;

export const ALL_EXPECTED_PUBLICATION_STATUSES = [
    "pending",
    "confirmed",
    "failed",
    "duplicateRunId",
] as const;
