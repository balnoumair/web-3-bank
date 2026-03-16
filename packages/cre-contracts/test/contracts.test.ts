import { describe, expect, it } from "vitest";

import { createRouteUpdatedEnvelope } from "../src";

describe("contracts package", () => {
  it("creates a message-only envelope", () => {
    const envelope = createRouteUpdatedEnvelope({
      runId: "run-123",
      customerId: "customer-1",
      recommendedChain: "base-sepolia",
      score: 0.82,
      timestamp: "2026-02-16T00:00:00.000Z",
    });

    expect(envelope.sourceChain).toBe("ethereum-sepolia");
    expect(envelope.destinationChain).toBe("base-sepolia");
    expect(envelope.payload.runId).toBe("run-123");
  });
});
