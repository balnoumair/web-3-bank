import { describe, expect, it } from "vitest";

import type {
  ScenarioName,
  ScoreWeights,
  ScoringV1ChainMetricsInput,
  ScoringV1InputMetricKey,
  ScoringV1ReasonCode,
} from "../src";

import {
  ActivationDecisionSchema,
  DEFAULT_SCORE_WEIGHTS,
  SCORING_V1_INPUT_METRIC_KEYS,
  SCORING_V1_METRIC_DIRECTION,
  SCORING_V1_REASON_CODES,
  SCORING_V1_SCENARIO_FALLBACKS,
  SCORING_V1_SCORE_DECIMALS,
  SCORING_V1_SCORE_TIE_EPSILON,
  SCORING_V1_TIE_BREAK_ORDER,
  SCORING_V1_WEIGHTS,
  ScoringV1ChainMetricsInputSchema,
  ScoringV1InputMetricKeySchema,
  ScoringV1ReasonCodeSchema,
} from "../src";
import { SCORING_V1_CASES } from "./fixtures/scoring-v1-cases";

type CompletedMetrics = {
  chain: string;
} & Record<ScoringV1InputMetricKey, number>;

type RankedScoring = {
  chain: string;
  score: number;
  raw: Record<ScoringV1InputMetricKey, number>;
  components: Record<keyof ScoreWeights, number>;
};

const componentKeyByMetricKey: Record<
  ScoringV1InputMetricKey,
  keyof ScoreWeights
> = {
  feeRaw: "fee",
  latencyRaw: "latency",
  reliabilityRaw: "reliability",
  liquidityRaw: "liquidity",
};

const missingReasonCodeByMetricKey: Record<
  ScoringV1InputMetricKey,
  ScoringV1ReasonCode
> = {
  feeRaw: "MISSING_FEE_RAW_FALLBACK",
  latencyRaw: "MISSING_LATENCY_RAW_FALLBACK",
  reliabilityRaw: "MISSING_RELIABILITY_RAW_FALLBACK",
  liquidityRaw: "MISSING_LIQUIDITY_RAW_FALLBACK",
};

const zeroRangeReasonCodeByMetricKey: Record<
  ScoringV1InputMetricKey,
  ScoringV1ReasonCode
> = {
  feeRaw: "ZERO_RANGE_FEE_RAW",
  latencyRaw: "ZERO_RANGE_LATENCY_RAW",
  reliabilityRaw: "ZERO_RANGE_RELIABILITY_RAW",
  liquidityRaw: "ZERO_RANGE_LIQUIDITY_RAW",
};

const scenarioReasonCodeByScenario: Record<ScenarioName, ScoringV1ReasonCode> = {
  normal: "SCENARIO_NORMAL",
  congested: "SCENARIO_CONGESTED",
};

function hasValidValue(value: number | null | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isTie(left: number, right: number): boolean {
  return Math.abs(left - right) <= SCORING_V1_SCORE_TIE_EPSILON;
}

function roundScore(value: number): number {
  return Number(value.toFixed(SCORING_V1_SCORE_DECIMALS));
}

function evaluateScoringV1Case(
  scenario: ScenarioName,
  metrics: ScoringV1ChainMetricsInput[],
): {
  ranked: Array<{ chain: string; score: number }>;
  recommendedChain: string;
  reasonCodes: ScoringV1ReasonCode[];
} {
  if (metrics.length === 0) {
    throw new Error("scoring v1 requires at least one chain");
  }

  const fallback = SCORING_V1_SCENARIO_FALLBACKS[scenario];

  const seenMissingMetricKeys = new Set<ScoringV1InputMetricKey>();
  const completedMetrics: CompletedMetrics[] = metrics.map((metricRow) => {
    const completed: CompletedMetrics = {
      chain: metricRow.chain,
      feeRaw: fallback.feeRaw,
      latencyRaw: fallback.latencyRaw,
      reliabilityRaw: fallback.reliabilityRaw,
      liquidityRaw: fallback.liquidityRaw,
    };

    for (const metricKey of SCORING_V1_INPUT_METRIC_KEYS) {
      const maybeValue = metricRow[metricKey];
      if (hasValidValue(maybeValue)) {
        completed[metricKey] = maybeValue;
      } else {
        seenMissingMetricKeys.add(metricKey);
      }
    }

    return completed;
  });

  const bounds = SCORING_V1_INPUT_METRIC_KEYS.reduce(
    (accumulator, metricKey) => {
      const values = completedMetrics.map((metricRow) => metricRow[metricKey]);
      accumulator[metricKey] = {
        min: Math.min(...values),
        max: Math.max(...values),
      };
      return accumulator;
    },
    {} as Record<ScoringV1InputMetricKey, { min: number; max: number }>,
  );

  const zeroRangeMetricKeys = new Set<ScoringV1InputMetricKey>();
  const rankedScoring: RankedScoring[] = completedMetrics.map((metricRow) => {
    const components: Record<keyof ScoreWeights, number> = {
      fee: 0,
      latency: 0,
      reliability: 0,
      liquidity: 0,
    };

    for (const metricKey of SCORING_V1_INPUT_METRIC_KEYS) {
      const { min, max } = bounds[metricKey];
      const range = max - min;

      const componentValue =
        Math.abs(range) <= SCORING_V1_SCORE_TIE_EPSILON
          ? 1
          : (() => {
            const normalized = (metricRow[metricKey] - min) / range;
            return SCORING_V1_METRIC_DIRECTION[metricKey] === "lower-better"
              ? 1 - normalized
              : normalized;
          })();

      if (Math.abs(range) <= SCORING_V1_SCORE_TIE_EPSILON) {
        zeroRangeMetricKeys.add(metricKey);
      }

      const componentKey = componentKeyByMetricKey[metricKey];
      components[componentKey] = componentValue;
    }

    const weightedScore =
      components.fee * SCORING_V1_WEIGHTS.fee +
      components.latency * SCORING_V1_WEIGHTS.latency +
      components.reliability * SCORING_V1_WEIGHTS.reliability +
      components.liquidity * SCORING_V1_WEIGHTS.liquidity;

    return {
      chain: metricRow.chain,
      score: roundScore(weightedScore),
      raw: {
        feeRaw: metricRow.feeRaw,
        latencyRaw: metricRow.latencyRaw,
        reliabilityRaw: metricRow.reliabilityRaw,
        liquidityRaw: metricRow.liquidityRaw,
      },
      components,
    };
  });

  rankedScoring.sort((left, right) => {
    if (!isTie(left.score, right.score)) {
      return right.score - left.score;
    }

    if (!isTie(left.raw.reliabilityRaw, right.raw.reliabilityRaw)) {
      return right.raw.reliabilityRaw - left.raw.reliabilityRaw;
    }

    if (!isTie(left.raw.latencyRaw, right.raw.latencyRaw)) {
      return left.raw.latencyRaw - right.raw.latencyRaw;
    }

    return left.chain.localeCompare(right.chain);
  });

  const reasonCodes: ScoringV1ReasonCode[] = [
    scenarioReasonCodeByScenario[scenario],
  ];

  for (const metricKey of SCORING_V1_INPUT_METRIC_KEYS) {
    if (seenMissingMetricKeys.has(metricKey)) {
      reasonCodes.push(missingReasonCodeByMetricKey[metricKey]);
    }
  }

  for (const metricKey of SCORING_V1_INPUT_METRIC_KEYS) {
    if (zeroRangeMetricKeys.has(metricKey)) {
      reasonCodes.push(zeroRangeReasonCodeByMetricKey[metricKey]);
    }
  }

  const topChain = rankedScoring[0];
  if (!topChain) {
    throw new Error("scoring v1 requires at least one scored chain");
  }

  const highestScore = topChain.score;
  const topScoreGroup = rankedScoring.filter((chainScore) =>
    isTie(chainScore.score, highestScore),
  );

  if (topScoreGroup.length > 1) {
    const maxReliability = Math.max(
      ...topScoreGroup.map((chainScore) => chainScore.raw.reliabilityRaw),
    );
    const reliabilityWinners = topScoreGroup.filter((chainScore) =>
      isTie(chainScore.raw.reliabilityRaw, maxReliability),
    );

    if (reliabilityWinners.length === 1) {
      reasonCodes.push("TIE_BREAK_RELIABILITY");
    } else {
      const minLatency = Math.min(
        ...reliabilityWinners.map((chainScore) => chainScore.raw.latencyRaw),
      );
      const latencyWinners = reliabilityWinners.filter((chainScore) =>
        isTie(chainScore.raw.latencyRaw, minLatency),
      );

      if (latencyWinners.length === 1) {
        reasonCodes.push("TIE_BREAK_LATENCY");
      } else {
        reasonCodes.push("TIE_BREAK_CHAIN");
      }
    }
  }

  return {
    ranked: rankedScoring.map((chainScore) => ({
      chain: chainScore.chain,
      score: chainScore.score,
    })),
    recommendedChain: topChain.chain,
    reasonCodes,
  };
}

describe("shared types package", () => {
  it("exports normalized default score weights", () => {
    const sum =
      DEFAULT_SCORE_WEIGHTS.fee +
      DEFAULT_SCORE_WEIGHTS.latency +
      DEFAULT_SCORE_WEIGHTS.reliability +
      DEFAULT_SCORE_WEIGHTS.liquidity;

    expect(sum).toBeCloseTo(1, 10);
  });

  it("keeps the scoring v1 weights in sync with defaults", () => {
    expect(SCORING_V1_WEIGHTS).toEqual(DEFAULT_SCORE_WEIGHTS);
  });

  it("publishes the scoring v1 tie-breaker order", () => {
    expect(SCORING_V1_TIE_BREAK_ORDER).toEqual([
      "reliabilityRawDesc",
      "latencyRawAsc",
      "chainLexAsc",
    ]);
  });

  it.each(SCORING_V1_CASES)(
    "matches scoring v1 fixture: $id",
    ({ scenario, metrics, expected }) => {
      const scoreResult = evaluateScoringV1Case(scenario, metrics);

      expect(scoreResult.recommendedChain).toBe(expected.recommendedChain);
      expect(scoreResult.ranked).toEqual(expected.ranked);
      expect(scoreResult.reasonCodes).toEqual(expected.reasonCodes);
    },
  );

  it("keeps fixture reason codes inside the published reason code set", () => {
    const fixtureReasonCodes = new Set(
      SCORING_V1_CASES.flatMap((fixture) => fixture.expected.reasonCodes),
    );

    expect(
      [...fixtureReasonCodes].every((code) =>
        SCORING_V1_REASON_CODES.includes(code),
      ),
    ).toBe(true);
  });

  describe("Zod Runtime Validation", () => {
    it("validates threshold-based activation output", () => {
      expect(
        ActivationDecisionSchema.parse({
          thresholdUsed: 0.6,
          activeChains: ["base-sepolia"],
          inactiveChains: ["arbitrum-sepolia"],
        }),
      ).toEqual({
        thresholdUsed: 0.6,
        activeChains: ["base-sepolia"],
        inactiveChains: ["arbitrum-sepolia"],
      });
    });

    it("validates valid reason codes", () => {
      expect(ScoringV1ReasonCodeSchema.parse("SCENARIO_NORMAL")).toBe(
        "SCENARIO_NORMAL",
      );
    });

    it("rejects invalid reason codes", () => {
      expect(() => ScoringV1ReasonCodeSchema.parse("INVALID_CODE")).toThrow();
    });

    it("validates valid metric keys", () => {
      expect(ScoringV1InputMetricKeySchema.parse("feeRaw")).toBe("feeRaw");
    });

    it("rejects invalid metric keys", () => {
      expect(() =>
        ScoringV1InputMetricKeySchema.parse("invalidMetric"),
      ).toThrow();
    });

    it("validates valid chain metrics input", () => {
      const input = {
        chain: "base-sepolia",
        feeRaw: 100,
        latencyRaw: 200,
        reliabilityRaw: 0.99,
        liquidityRaw: 0.5,
      };
      expect(() => ScoringV1ChainMetricsInputSchema.parse(input)).not.toThrow();
    });

    it("allows optional/nullable fields in chain metrics input", () => {
      const input = {
        chain: "base-sepolia",
        feeRaw: null,
        // latency/reliability/liquidity missing (optional)
      };
      expect(() => ScoringV1ChainMetricsInputSchema.parse(input)).not.toThrow();
    });

    it("rejects invalid chain metrics input", () => {
      const input = {
        chain: 123, // invalid type
        feeRaw: "high", // invalid type
      };
      expect(() => ScoringV1ChainMetricsInputSchema.parse(input)).toThrow();
    });
  });
});
