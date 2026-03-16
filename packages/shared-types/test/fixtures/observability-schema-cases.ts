import type {
  CcipDeliveryRecord,
  CcipDeliveryStatus,
  ScoringRunRecord,
} from "../../src";

// ── Happy path: started → scored → ccipSent → ccipConfirmed ─────────

const happyRun: ScoringRunRecord = {
  runId: "run-happy-001",
  customerId: "customer-1",
  requestId: "req-40-happy-001",
  configVersion: 12,
  snapshotUpdatedAt: "2026-02-17T11:59:00.000Z",
  status: "ccipConfirmed",
  timestamp: "2026-02-17T12:00:00.000Z",
  scenario: "normal",
  recommendedChain: "base-sepolia",
  score: 0.82,
  reasonCodes: ["SCENARIO_NORMAL"],
  attemptCount: 1,
  errorCode: null,
  errorMessage: null,
};

const happyDelivery: CcipDeliveryRecord = {
  deliveryId: "del-happy-001",
  runId: "run-happy-001",
  ccipMessageId: "0xabc123",
  sourceChain: "ethereum-sepolia",
  destinationChain: "base-sepolia",
  status: "confirmed",
  attemptCount: 1,
  errorCode: null,
  errorMessage: null,
  timestamp: "2026-02-17T12:00:05.000Z",
};

// ── Failure path: started → scored → ccipSent → partialFailure ──────

const failureRun: ScoringRunRecord = {
  runId: "run-fail-001",
  customerId: "customer-2",
  requestId: "req-40-fail-001",
  configVersion: 18,
  snapshotUpdatedAt: "2026-02-17T12:00:30.000Z",
  status: "partialFailure",
  timestamp: "2026-02-17T12:01:00.000Z",
  scenario: "congested",
  recommendedChain: "base-sepolia",
  score: 0.65,
  reasonCodes: ["SCENARIO_CONGESTED"],
  attemptCount: 3,
  errorCode: "CCIP_SEND_TIMEOUT",
  errorMessage: "All 3 CCIP send attempts failed",
};

const failureDelivery: CcipDeliveryRecord = {
  deliveryId: "del-fail-003",
  runId: "run-fail-001",
  ccipMessageId: null,
  sourceChain: "ethereum-sepolia",
  destinationChain: "base-sepolia",
  status: "failed",
  attemptCount: 3,
  errorCode: "CCIP_SEND_TIMEOUT",
  errorMessage: "Timeout after 30s backoff",
  timestamp: "2026-02-17T12:01:50.000Z",
};

// ── Overlap: skippedOverlap with minimal fields ─────────────────────

const overlapRun: ScoringRunRecord = {
  runId: "run-overlap-001",
  customerId: "customer-1",
  requestId: null,
  configVersion: null,
  snapshotUpdatedAt: null,
  status: "skippedOverlap",
  timestamp: "2026-02-17T12:02:00.000Z",
  scenario: "normal",
  recommendedChain: null,
  score: null,
  reasonCodes: [],
  attemptCount: 0,
  errorCode: null,
  errorMessage: null,
};

export const OBSERVABILITY_CASES = {
  happyPath: { run: happyRun, deliveries: [happyDelivery] },
  failurePath: { run: failureRun, deliveries: [failureDelivery] },
  overlapPath: { run: overlapRun, deliveries: [] },
} as const;

export const ALL_EXPECTED_DELIVERY_STATUSES: CcipDeliveryStatus[] = [
  "pending",
  "sent",
  "confirmed",
  "failed",
];
