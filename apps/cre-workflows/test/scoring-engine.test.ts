import { describe, expect, it } from "vitest";

import { classifyActivationByThreshold, scoreRunV1 } from "../src";
import { SCORING_ENGINE_CASES } from "./fixtures/scoring-engine-cases";

describe("scoring engine", () => {
  it.each(SCORING_ENGINE_CASES)("matches deterministic fixture: $id", (testCase) => {
    const result = scoreRunV1({
      runId: `run-${testCase.id}`,
      customerId: "customer-16",
      scenario: testCase.scenario,
      allowedChains: testCase.metrics.map((metric) => metric.chain),
      chainMetrics: testCase.metrics,
    });

    expect(result.recommendedChain).toBe(testCase.expected.recommendedChain);
    expect(result.ranked.map(({ chain, score }) => ({ chain, score }))).toEqual(
      testCase.expected.ranked,
    );
    expect(result.reasonCodes).toEqual(testCase.expected.reasonCodes);
  });

  it("produces identical output for identical input", () => {
    const input = {
      runId: "run-deterministic-001",
      customerId: "customer-16",
      scenario: "normal" as const,
      allowedChains: ["base-sepolia", "arbitrum-sepolia"],
      chainMetrics: [
        {
          chain: "base-sepolia",
          feeRaw: 20,
          latencyRaw: 170,
          reliabilityRaw: 0.995,
          liquidityRaw: 0.87,
        },
        {
          chain: "arbitrum-sepolia",
          feeRaw: 28,
          latencyRaw: 190,
          reliabilityRaw: 0.991,
          liquidityRaw: 0.84,
        },
      ],
    };

    const first = scoreRunV1(input);
    const second = scoreRunV1(input);

    expect(second).toEqual(first);
  });

  it("scores only allowed chains and uses fallback for missing allowed chain metrics", () => {
    const result = scoreRunV1({
      runId: "run-allowed-filter-001",
      customerId: "customer-16",
      scenario: "normal",
      allowedChains: ["base-sepolia", "optimism-sepolia"],
      chainMetrics: [
        {
          chain: "base-sepolia",
          feeRaw: 20,
          latencyRaw: 180,
          reliabilityRaw: 0.99,
          liquidityRaw: 0.88,
        },
        {
          chain: "arbitrum-sepolia",
          feeRaw: 15,
          latencyRaw: 160,
          reliabilityRaw: 0.98,
          liquidityRaw: 0.85,
        },
      ],
    });

    expect(result.ranked.map((entry) => entry.chain)).toEqual([
      "base-sepolia",
      "optimism-sepolia",
    ]);
    expect(result.reasonCodes).toEqual([
      "SCENARIO_NORMAL",
      "MISSING_FEE_RAW_FALLBACK",
      "MISSING_LATENCY_RAW_FALLBACK",
      "MISSING_RELIABILITY_RAW_FALLBACK",
      "MISSING_LIQUIDITY_RAW_FALLBACK",
    ]);
  });

  it("classifies active and inactive chains using an inclusive threshold", () => {
    const decision = classifyActivationByThreshold({
      threshold: 0.6,
      ranked: [
        {
          chain: "base-sepolia",
          score: 0.61,
          components: {
            fee: 0.2,
            latency: 0.2,
            reliability: 0.15,
            liquidity: 0.06,
          },
        },
        {
          chain: "arbitrum-sepolia",
          score: 0.6,
          components: {
            fee: 0.2,
            latency: 0.2,
            reliability: 0.15,
            liquidity: 0.05,
          },
        },
        {
          chain: "optimism-sepolia",
          score: 0.59,
          components: {
            fee: 0.19,
            latency: 0.2,
            reliability: 0.15,
            liquidity: 0.05,
          },
        },
      ],
    });

    expect(decision).toEqual({
      thresholdUsed: 0.6,
      activeChains: ["base-sepolia", "arbitrum-sepolia"],
      inactiveChains: ["optimism-sepolia"],
    });
  });
});
