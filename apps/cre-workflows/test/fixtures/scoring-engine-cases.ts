import type {
  ScenarioName,
  ScoringV1ChainMetricsInput,
  ScoringV1ReasonCode,
} from "@repo/shared-types";

export type ScoringEngineCase = {
  id: string;
  scenario: ScenarioName;
  metrics: ScoringV1ChainMetricsInput[];
  expected: {
    recommendedChain: string;
    ranked: Array<{
      chain: string;
      score: number;
    }>;
    reasonCodes: ScoringV1ReasonCode[];
  };
};

export const SCORING_ENGINE_CASES: ScoringEngineCase[] = [
  {
    id: "normal-baseline-ranking",
    scenario: "normal",
    metrics: [
      {
        chain: "base-sepolia",
        feeRaw: 25,
        latencyRaw: 180,
        reliabilityRaw: 0.992,
        liquidityRaw: 0.88,
      },
      {
        chain: "arbitrum-sepolia",
        feeRaw: 30,
        latencyRaw: 150,
        reliabilityRaw: 0.985,
        liquidityRaw: 0.91,
      },
      {
        chain: "optimism-sepolia",
        feeRaw: 20,
        latencyRaw: 210,
        reliabilityRaw: 0.978,
        liquidityRaw: 0.86,
      },
    ],
    expected: {
      recommendedChain: "base-sepolia",
      ranked: [
        { chain: "base-sepolia", score: 0.615 },
        { chain: "arbitrum-sepolia", score: 0.525 },
        { chain: "optimism-sepolia", score: 0.35 },
      ],
      reasonCodes: ["SCENARIO_NORMAL"],
    },
  },
  {
    id: "score-tie-resolved-by-reliability",
    scenario: "normal",
    metrics: [
      {
        chain: "alpha-sepolia",
        feeRaw: 10,
        latencyRaw: 300,
        reliabilityRaw: 0.96,
        liquidityRaw: 1,
      },
      {
        chain: "beta-sepolia",
        feeRaw: 30,
        latencyRaw: 100,
        reliabilityRaw: 1,
        liquidityRaw: 1,
      },
      {
        chain: "gamma-sepolia",
        feeRaw: 20,
        latencyRaw: 200,
        reliabilityRaw: 0.8,
        liquidityRaw: 0.5,
      },
    ],
    expected: {
      recommendedChain: "beta-sepolia",
      ranked: [
        { chain: "beta-sepolia", score: 0.65 },
        { chain: "alpha-sepolia", score: 0.65 },
        { chain: "gamma-sepolia", score: 0.325 },
      ],
      reasonCodes: ["SCENARIO_NORMAL", "TIE_BREAK_RELIABILITY"],
    },
  },
  {
    id: "missing-metrics-use-scenario-fallbacks",
    scenario: "normal",
    metrics: [
      {
        chain: "base-sepolia",
        latencyRaw: 210,
        reliabilityRaw: 0.98,
        liquidityRaw: 0.72,
      },
      {
        chain: "arbitrum-sepolia",
        feeRaw: 42,
        liquidityRaw: 0.81,
      },
      {
        chain: "optimism-sepolia",
        feeRaw: 38,
        latencyRaw: 260,
        reliabilityRaw: 0.96,
      },
    ],
    expected: {
      recommendedChain: "base-sepolia",
      ranked: [
        { chain: "base-sepolia", score: 0.9 },
        { chain: "arbitrum-sepolia", score: 0.4025 },
        { chain: "optimism-sepolia", score: 0.233333 },
      ],
      reasonCodes: [
        "SCENARIO_NORMAL",
        "MISSING_FEE_RAW_FALLBACK",
        "MISSING_LATENCY_RAW_FALLBACK",
        "MISSING_RELIABILITY_RAW_FALLBACK",
        "MISSING_LIQUIDITY_RAW_FALLBACK",
      ],
    },
  },
];
