import { describe, expect, it } from "vitest";

import {
  RUN_TERMINAL_STATUSES,
  decideRunStart,
  guardRunStatusTransition,
} from "../src";
import {
  INVALID_TRANSITION_CASES,
  VALID_TRANSITION_CASES,
} from "./fixtures/run-state-machine-cases";

describe("run state machine", () => {
  const runId = "run-015";
  const customerId = "customer-015";
  const timestamp = "2026-02-19T12:00:00.000Z";

  it("exports terminal statuses for deterministic checks", () => {
    expect(RUN_TERMINAL_STATUSES).toEqual([
      "ccipConfirmed",
      "partialFailure",
      "skippedOverlap",
    ]);
  });

  it.each(VALID_TRANSITION_CASES)("accepts valid transition: $id", (testCase) => {
    const result = guardRunStatusTransition({
      runId,
      customerId,
      fromStatus: testCase.fromStatus,
      toStatus: testCase.toStatus,
      timestamp,
    });

    expect(result.accepted).toBe(true);
    expect(result.log.level).toBe("info");
    expect(result.log.event).toBe("run.transition.accepted");
  });

  it.each(INVALID_TRANSITION_CASES)(
    "rejects invalid transition: $id",
    (testCase) => {
      const result = guardRunStatusTransition({
        runId,
        customerId,
        fromStatus: testCase.fromStatus,
        toStatus: testCase.toStatus,
        timestamp,
      });

      expect(result.accepted).toBe(false);
      if (!result.accepted) {
        expect(result.errorCode).toBe(testCase.errorCode);
        expect(result.log.level).toBe("warn");
        expect(result.log.event).toBe("run.transition.rejected");
      }
    },
  );

  it("marks overlap tick as skippedOverlap", () => {
    const decision = decideRunStart({
      runId,
      customerId,
      hasActiveRun: true,
      timestamp,
    });

    expect(decision.status).toBe("skippedOverlap");
    expect(decision.log.event).toBe("run.overlap.skipped");
  });

  it("marks non-overlap tick as started", () => {
    const decision = decideRunStart({
      runId,
      customerId,
      hasActiveRun: false,
      timestamp,
    });

    expect(decision.status).toBe("started");
    expect(decision.log.event).toBe("run.start.accepted");
  });
});
