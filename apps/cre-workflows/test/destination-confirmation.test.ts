import { describe, expect, it } from "vitest";

import { handleDestinationConfirmation } from "../src";

describe("destination confirmation", () => {
  it("does not mark run success before confirmation arrives", () => {
    const result = handleDestinationConfirmation({
      runId: "run-022-await",
      customerId: "customer-022",
      currentStatus: "ccipSent",
      ccipMessageId: "0xmessage-await",
      confirmationTimeoutAt: "2026-02-21T15:00:00.000Z",
      now: "2026-02-21T14:59:59.000Z",
      confirmationEvent: null,
    });

    expect(result).toEqual({
      status: "awaitingConfirmation",
      runId: "run-022-await",
      ccipMessageId: "0xmessage-await",
      timeoutAt: "2026-02-21T15:00:00.000Z",
    });
  });

  it("ends in partialFailure when confirmation timeout is reached", () => {
    const result = handleDestinationConfirmation({
      runId: "run-022-timeout",
      customerId: "customer-022",
      currentStatus: "ccipSent",
      ccipMessageId: "0xmessage-timeout",
      confirmationTimeoutAt: "2026-02-21T15:00:00.000Z",
      now: "2026-02-21T15:00:00.000Z",
      confirmationEvent: null,
    });

    expect(result.status).toBe("partialFailure");
    if (result.status === "partialFailure") {
      expect(result.runId).toBe("run-022-timeout");
      expect(result.errorCode).toBe("CCIP_CONFIRMATION_TIMEOUT");
      expect(result.transition.accepted).toBe(true);
      expect(result.transition.toStatus).toBe("partialFailure");
    }
  });

  it("links confirmation event data to the originating runId", () => {
    const result = handleDestinationConfirmation({
      runId: "run-022-confirmed",
      customerId: "customer-022",
      currentStatus: "ccipSent",
      ccipMessageId: "0xmessage-confirmed",
      confirmationTimeoutAt: "2026-02-21T15:00:00.000Z",
      now: "2026-02-21T14:30:00.000Z",
      confirmationEvent: {
        runId: "run-022-confirmed",
        ccipMessageId: "0xmessage-confirmed",
        destinationTransactionHash: "0xdestination-tx",
        observedAt: "2026-02-21T14:29:59.000Z",
      },
    });

    expect(result.status).toBe("ccipConfirmed");
    if (result.status === "ccipConfirmed") {
      expect(result.runId).toBe("run-022-confirmed");
      expect(result.transition.accepted).toBe(true);
      expect(result.transition.toStatus).toBe("ccipConfirmed");
      expect(result.confirmation.runId).toBe("run-022-confirmed");
      expect(result.confirmation.ccipMessageId).toBe("0xmessage-confirmed");
    }
  });

  it("rejects confirmation handling when run is not in ccipSent state", () => {
    const result = handleDestinationConfirmation({
      runId: "run-022-invalid-state",
      customerId: "customer-022",
      currentStatus: "scored",
      ccipMessageId: "0xmessage-invalid",
      confirmationTimeoutAt: "2026-02-21T15:00:00.000Z",
      now: "2026-02-21T14:30:00.000Z",
      confirmationEvent: {
        runId: "run-022-invalid-state",
        ccipMessageId: "0xmessage-invalid",
        destinationTransactionHash: "0xdestination-tx",
        observedAt: "2026-02-21T14:29:59.000Z",
      },
    });

    expect(result).toEqual({
      status: "rejected",
      runId: "run-022-invalid-state",
      errorCode: "RUN_STATUS_TRANSITION_INVALID",
      errorMessage: "destination confirmation can only be handled from 'ccipSent'",
    });
  });
});
