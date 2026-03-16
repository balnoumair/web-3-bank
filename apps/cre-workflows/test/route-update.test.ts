import { describe, expect, it } from "vitest";

import type { ScoreResult } from "@repo/shared-types";

import { toRouteUpdated } from "../src";

describe("cre-workflows package", () => {
  it("maps score results into a RouteUpdated payload", () => {
    const scoreResult: ScoreResult = {
      runId: "run-001",
      customerId: "customer-abc",
      ranked: [
        {
          chain: "base-sepolia",
          score: 0.91,
          components: {
            fee: 0.32,
            latency: 0.25,
            reliability: 0.25,
            liquidity: 0.09,
          },
        },
      ],
      recommendedChain: "base-sepolia",
      confidence: 0.91,
      reasonCodes: ["SCENARIO_NORMAL"],
    };

    const payload = toRouteUpdated(scoreResult);

    expect(payload.runId).toBe("run-001");
    expect(payload.customerId).toBe("customer-abc");
    expect(payload.recommendedChain).toBe("base-sepolia");
    expect(payload.score).toBe(0.91);
    expect(new Date(payload.timestamp).toISOString()).toBe(payload.timestamp);
  });
});
