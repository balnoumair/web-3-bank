import type { ScoringV1ChainMetricsInput } from "@repo/shared-types";

export const HAPPY_PATH_VALIDATION_CASE = {
  customerId: "customer-021",
  scenario: "normal" as const,
  activationThreshold: 0.6,
  allowedChains: ["base-sepolia", "arbitrum-sepolia", "optimism-sepolia"],
  chainMetrics: [
    {
      chain: "base-sepolia",
      feeRaw: 21,
      latencyRaw: 165,
      reliabilityRaw: 0.996,
      liquidityRaw: 0.9,
    },
    {
      chain: "arbitrum-sepolia",
      feeRaw: 29,
      latencyRaw: 140,
      reliabilityRaw: 0.988,
      liquidityRaw: 0.87,
    },
    {
      chain: "optimism-sepolia",
      feeRaw: 25,
      latencyRaw: 205,
      reliabilityRaw: 0.981,
      liquidityRaw: 0.82,
    },
  ] satisfies ScoringV1ChainMetricsInput[],
};
