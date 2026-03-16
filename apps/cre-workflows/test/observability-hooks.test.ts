import { describe, expect, it } from "vitest";

import {
  createObservabilityHooks,
  type RunTraceEvent,
} from "../src";

describe("observability hooks", () => {
  it("emits required trace points in lifecycle order", () => {
    const traceEvents: RunTraceEvent[] = [];

    const hooks = createObservabilityHooks({
      log: (event) => {
        traceEvents.push(event);
      },
      saveRun: () => {},
      saveDelivery: () => {},
    });

    hooks.onStarted({
      runId: "run-019-happy",
      customerId: "customer-019",
      scenario: "normal",
      requestId: "req-40-001",
      configVersion: 12,
      snapshotUpdatedAt: "2026-02-21T15:59:59.000Z",
      timestamp: "2026-02-21T16:00:00.000Z",
    });

    hooks.onScored({
      runId: "run-019-happy",
      customerId: "customer-019",
      scenario: "normal",
      previousStatus: "started",
      recommendedChain: "base-sepolia",
      score: 0.88,
      reasonCodes: ["SCENARIO_NORMAL"],
      requestId: "req-40-001",
      configVersion: 12,
      snapshotUpdatedAt: "2026-02-21T15:59:59.000Z",
      timestamp: "2026-02-21T16:00:01.000Z",
    });

    hooks.onCcipSent({
      runId: "run-019-happy",
      customerId: "customer-019",
      scenario: "normal",
      previousStatus: "scored",
      deliveryId: "delivery-019-happy",
      recommendedChain: "base-sepolia",
      score: 0.88,
      reasonCodes: ["SCENARIO_NORMAL"],
      attemptCount: 1,
      ccipMessageId: "0xmessage-019",
      requestId: "req-40-001",
      configVersion: 12,
      snapshotUpdatedAt: "2026-02-21T15:59:59.000Z",
      timestamp: "2026-02-21T16:00:02.000Z",
    });

    hooks.onTerminal({
      runId: "run-019-happy",
      customerId: "customer-019",
      scenario: "normal",
      previousStatus: "ccipSent",
      terminalStatus: "ccipConfirmed",
      recommendedChain: "base-sepolia",
      score: 0.88,
      reasonCodes: ["SCENARIO_NORMAL"],
      attemptCount: 1,
      deliveryId: "delivery-019-happy",
      ccipMessageId: "0xmessage-019",
      requestId: "req-40-001",
      configVersion: 12,
      snapshotUpdatedAt: "2026-02-21T15:59:59.000Z",
      timestamp: "2026-02-21T16:00:03.000Z",
    });

    expect(traceEvents.map((event) => event.tracePoint)).toEqual([
      "started",
      "scored",
      "ccipSent",
      "terminal",
    ]);

    for (const event of traceEvents) {
      expect(event.runId).toBe("run-019-happy");
      expect(event.customerId).toBe("customer-019");
      expect(event.requestId).toBe("req-40-001");
      expect(event.configVersion).toBe(12);
      expect(event.snapshotUpdatedAt).toBe("2026-02-21T15:59:59.000Z");
      expect(event.status).toBeDefined();
      expect(event.timestamp).toMatch(/^2026-02-21T16:00:0[0-3]\.000Z$/);
    }
  });

  it("aligns persistence updates with run state machine transitions", () => {
    const hooks = createObservabilityHooks({
      log: () => {},
      saveRun: () => {},
      saveDelivery: () => {},
    });

    expect(() =>
      hooks.onCcipSent({
        runId: "run-019-invalid",
        customerId: "customer-019",
        scenario: "normal",
        previousStatus: "started",
        deliveryId: "delivery-019-invalid",
        recommendedChain: "base-sepolia",
        score: 0.88,
        reasonCodes: ["SCENARIO_NORMAL"],
        attemptCount: 1,
        ccipMessageId: "0xmessage-019-invalid",
        timestamp: "2026-02-21T16:05:00.000Z",
      }),
    ).toThrow("RUN_STATUS_TRANSITION_INVALID");
  });

  it("includes reason and failure metadata on error terminal paths", () => {
    const traceEvents: RunTraceEvent[] = [];
    const savedRuns: Array<{ errorCode: string | null; errorMessage: string | null }> = [];
    const savedDeliveries: Array<{ errorCode: string | null; errorMessage: string | null }> = [];

    const hooks = createObservabilityHooks({
      log: (event) => {
        traceEvents.push(event);
      },
      saveRun: (record) => {
        savedRuns.push({
          errorCode: record.errorCode,
          errorMessage: record.errorMessage,
        });
      },
      saveDelivery: (record) => {
        savedDeliveries.push({
          errorCode: record.errorCode,
          errorMessage: record.errorMessage,
        });
      },
    });

    hooks.onTerminal({
      runId: "run-019-failure",
      customerId: "customer-019",
      scenario: "congested",
      previousStatus: "ccipSent",
      terminalStatus: "partialFailure",
      recommendedChain: "base-sepolia",
      score: 0.54,
      reasonCodes: ["SCENARIO_CONGESTED"],
      attemptCount: 3,
      deliveryId: "delivery-019-failure",
      ccipMessageId: "0xmessage-019-failure",
      timestamp: "2026-02-21T16:10:00.000Z",
      errorCode: "CCIP_CONFIRMATION_TIMEOUT",
      errorMessage: "destination confirmation timed out",
    });

    const terminalEvent = traceEvents[0];
    expect(terminalEvent.tracePoint).toBe("terminal");
    expect(terminalEvent.status).toBe("partialFailure");
    expect(terminalEvent.errorCode).toBe("CCIP_CONFIRMATION_TIMEOUT");
    expect(terminalEvent.errorMessage).toBe("destination confirmation timed out");
    expect(terminalEvent.reasonCodes).toEqual(["SCENARIO_CONGESTED"]);

    expect(savedRuns[0]).toEqual({
      errorCode: "CCIP_CONFIRMATION_TIMEOUT",
      errorMessage: "destination confirmation timed out",
    });
    expect(savedDeliveries[0]).toEqual({
      errorCode: "CCIP_CONFIRMATION_TIMEOUT",
      errorMessage: "destination confirmation timed out",
    });
  });
});
