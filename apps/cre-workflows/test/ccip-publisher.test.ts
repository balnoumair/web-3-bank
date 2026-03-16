import { describe, expect, it } from "vitest";

import { publishRouteUpdateWithRetry } from "../src";

describe("ccip publisher", () => {
  it("sends at most 3 attempts with configured backoff and preserves runId", async () => {
    const sendCalls: Array<{ runId: string; attempt: number }> = [];
    const backoffCalls: number[] = [];

    const result = await publishRouteUpdateWithRetry({
      routeUpdated: {
        runId: "run-017-retry",
        customerId: "customer-017",
        recommendedChain: "base-sepolia",
        score: 0.82,
        timestamp: "2026-02-21T00:00:00.000Z",
      },
      send: async ({ envelope, attempt }) => {
        sendCalls.push({ runId: envelope.payload.runId, attempt });
        if (attempt < 3) {
          throw new Error(`send failed at attempt ${attempt}`);
        }

        return { ccipMessageId: "0xmessage-017" };
      },
      onBackoff: async (delayMs) => {
        backoffCalls.push(delayMs);
      },
    });

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.status).toBe("ccipSent");
      expect(result.runId).toBe("run-017-retry");
      expect(result.attemptCount).toBe(3);
      expect(result.ccipMessageId).toBe("0xmessage-017");
      expect(result.envelope.sourceChain).toBe("ethereum-sepolia");
      expect(result.envelope.destinationChain).toBe("base-sepolia");
      expect(result.envelope.payload.runId).toBe("run-017-retry");
    }

    expect(sendCalls).toEqual([
      { runId: "run-017-retry", attempt: 1 },
      { runId: "run-017-retry", attempt: 2 },
      { runId: "run-017-retry", attempt: 3 },
    ]);
    expect(backoffCalls).toEqual([1000, 2000]);
  });

  it("blocks duplicate publish for the same runId", async () => {
    const sendCalls: number[] = [];

    const idempotencyStore = new Set<string>();

    const first = await publishRouteUpdateWithRetry({
      routeUpdated: {
        runId: "run-017-duplicate",
        customerId: "customer-017",
        recommendedChain: "base-sepolia",
        score: 0.71,
        timestamp: "2026-02-21T00:01:00.000Z",
      },
      idempotencyStore,
      send: async () => {
        sendCalls.push(1);
        return { ccipMessageId: "0xmessage-first" };
      },
    });

    const duplicate = await publishRouteUpdateWithRetry({
      routeUpdated: {
        runId: "run-017-duplicate",
        customerId: "customer-017",
        recommendedChain: "base-sepolia",
        score: 0.71,
        timestamp: "2026-02-21T00:01:00.000Z",
      },
      idempotencyStore,
      send: async () => {
        sendCalls.push(2);
        return { ccipMessageId: "0xmessage-second" };
      },
    });

    expect(first.ok).toBe(true);
    expect(duplicate).toEqual({
      ok: false,
      runId: "run-017-duplicate",
      status: "duplicateBlocked",
      attemptCount: 0,
      errorCode: "CCIP_DUPLICATE_RUN_ID",
      errorMessage: "duplicate publish blocked for runId 'run-017-duplicate'",
    });
    expect(sendCalls).toEqual([1]);
  });

  it("returns partialFailure with reason after retries are exhausted", async () => {
    const backoffCalls: number[] = [];

    const result = await publishRouteUpdateWithRetry({
      routeUpdated: {
        runId: "run-017-fail",
        customerId: "customer-017",
        recommendedChain: "base-sepolia",
        score: 0.63,
        timestamp: "2026-02-21T00:02:00.000Z",
      },
      send: async () => {
        throw new Error("router timeout");
      },
      onBackoff: async (delayMs) => {
        backoffCalls.push(delayMs);
      },
    });

    expect(result).toEqual({
      ok: false,
      runId: "run-017-fail",
      status: "partialFailure",
      attemptCount: 3,
      errorCode: "CCIP_SEND_RETRIES_EXHAUSTED",
      errorMessage: "router timeout",
    });
    expect(backoffCalls).toEqual([1000, 2000]);
  });
});
