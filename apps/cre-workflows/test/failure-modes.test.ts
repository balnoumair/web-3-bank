import { describe, expect, it } from "vitest";

import {
  decideRunStart,
  guardRunStatusTransition,
  handleDestinationConfirmation,
  publishRouteUpdateWithRetry,
} from "../src";
import { FAILURE_MODE_CASES } from "./fixtures/failure-mode-cases";

describe("CRE Evaluator failure modes", () => {
  it("send failure after retries ends in partialFailure with transition and reason", async () => {
    const publishResult = await publishRouteUpdateWithRetry({
      routeUpdated: {
        runId: "run-023-send-failure",
        customerId: "customer-023",
        recommendedChain: "base-sepolia",
        score: 0.72,
        timestamp: "2026-02-21T17:00:00.000Z",
      },
      send: async () => {
        throw new Error("router unavailable");
      },
    });

    expect(publishResult.ok).toBe(false);
    if (!publishResult.ok) {
      expect(publishResult.status).toBe("partialFailure");
      expect(publishResult.errorCode).toBe("CCIP_SEND_RETRIES_EXHAUSTED");
      expect(publishResult.errorMessage).toBe("router unavailable");
      expect(publishResult.attemptCount).toBe(3);

      const transition = guardRunStatusTransition({
        runId: publishResult.runId,
        customerId: "customer-023",
        fromStatus: "ccipSent",
        toStatus: "partialFailure",
      });

      expect(transition.accepted).toBe(true);
      if (transition.accepted) {
        expect(transition.toStatus).toBe("partialFailure");
      }
    }
  });

  it("confirmation timeout ends in partialFailure with transition and reason", () => {
    const confirmationResult = handleDestinationConfirmation({
      runId: "run-023-timeout",
      customerId: "customer-023",
      currentStatus: "ccipSent",
      ccipMessageId: "0xmessage-023-timeout",
      confirmationTimeoutAt: "2026-02-21T17:10:00.000Z",
      now: "2026-02-21T17:10:00.000Z",
      confirmationEvent: null,
    });

    expect(confirmationResult.status).toBe("partialFailure");
    if (confirmationResult.status === "partialFailure") {
      expect(confirmationResult.errorCode).toBe("CCIP_CONFIRMATION_TIMEOUT");
      expect(confirmationResult.errorMessage).toContain("timed out");
      expect(confirmationResult.transition.accepted).toBe(true);
      expect(confirmationResult.transition.toStatus).toBe("partialFailure");
    }
  });

  it("overlap tick ends in skippedOverlap", () => {
    const startDecision = decideRunStart({
      runId: "run-023-overlap",
      customerId: "customer-023",
      hasActiveRun: true,
      timestamp: "2026-02-21T17:20:00.000Z",
    });

    expect(startDecision.status).toBe("skippedOverlap");
    expect(startDecision.log.event).toBe("run.overlap.skipped");
    expect(startDecision.log.toStatus).toBe("skippedOverlap");
    expect(startDecision.log.errorCode).toBeNull();
    expect(startDecision.log.errorMessage).toBeNull();
  });

  it("keeps expected terminal statuses and reason mappings in fixtures", () => {
    expect(FAILURE_MODE_CASES).toEqual([
      {
        id: "send-failure-after-retries",
        expectedTerminalStatus: "partialFailure",
        expectedErrorCode: "CCIP_SEND_RETRIES_EXHAUSTED",
      },
      {
        id: "confirmation-timeout",
        expectedTerminalStatus: "partialFailure",
        expectedErrorCode: "CCIP_CONFIRMATION_TIMEOUT",
      },
      {
        id: "overlap-skip",
        expectedTerminalStatus: "skippedOverlap",
        expectedErrorCode: null,
      },
    ]);
  });
});
