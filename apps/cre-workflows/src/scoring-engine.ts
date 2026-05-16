import type {
  ActivationDecision,
  RankedChain,
  ScenarioName,
  ScoreResult,
  ScoringV1ChainMetricsInput,
  ScoringV1InputMetricKey,
  ScoringV1ReasonCode,
} from "@repo/shared-types";
import {
  SCORING_V1_INPUT_METRIC_KEYS,
  SCORING_V1_METRIC_DIRECTION,
  SCORING_V1_SCENARIO_FALLBACKS,
  SCORING_V1_SCORE_DECIMALS,
  SCORING_V1_SCORE_TIE_EPSILON,
  SCORING_V1_WEIGHTS,
} from "@repo/shared-types";

type ScoreRunV1Args = {
  runId: string;
  customerId: string;
  scenario: ScenarioName;
  allowedChains: string[];
  decommissionedChains?: string[];
  chainMetrics: ScoringV1ChainMetricsInput[];
};

type CompletedChainMetrics = {
  chain: string;
} & Record<ScoringV1InputMetricKey, number>;

type RankedScoring = {
  chain: string;
  score: number;
  components: RankedChain["components"];
  raw: Record<ScoringV1InputMetricKey, number>;
};

const componentKeyByMetricKey: Record<
  ScoringV1InputMetricKey,
  keyof RankedChain["components"]
> = {
  feeRaw: "fee",
  latencyRaw: "latency",
  reliabilityRaw: "reliability",
  liquidityRaw: "liquidity",
};

const scenarioReasonCodeByScenario: Record<ScenarioName, ScoringV1ReasonCode> =
  {
    normal: "SCENARIO_NORMAL",
    congested: "SCENARIO_CONGESTED",
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

function hasValidValue(value: number | null | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isTie(left: number, right: number): boolean {
  return Math.abs(left - right) <= SCORING_V1_SCORE_TIE_EPSILON;
}

function roundScore(value: number): number {
  return Number(value.toFixed(SCORING_V1_SCORE_DECIMALS));
}

function buildCompletedMetrics(args: {
  scenario: ScenarioName;
  allowedChains: string[];
  chainMetrics: ScoringV1ChainMetricsInput[];
}): {
  completedMetrics: CompletedChainMetrics[];
  seenMissingMetricKeys: Set<ScoringV1InputMetricKey>;
} {
  const fallback = SCORING_V1_SCENARIO_FALLBACKS[args.scenario];
  const chainMetricsByChain = new Map<string, ScoringV1ChainMetricsInput>();

  for (const metricRow of args.chainMetrics) {
    if (!chainMetricsByChain.has(metricRow.chain)) {
      chainMetricsByChain.set(metricRow.chain, metricRow);
    }
  }

  const seenMissingMetricKeys = new Set<ScoringV1InputMetricKey>();
  const completedMetrics: CompletedChainMetrics[] = args.allowedChains.map(
    (chain) => {
      const chainMetricRow = chainMetricsByChain.get(chain);
      const completed: CompletedChainMetrics = {
        chain,
        feeRaw: fallback.feeRaw,
        latencyRaw: fallback.latencyRaw,
        reliabilityRaw: fallback.reliabilityRaw,
        liquidityRaw: fallback.liquidityRaw,
      };

      for (const metricKey of SCORING_V1_INPUT_METRIC_KEYS) {
        const maybeValue = chainMetricRow?.[metricKey];
        if (hasValidValue(maybeValue)) {
          completed[metricKey] = maybeValue;
        } else {
          seenMissingMetricKeys.add(metricKey);
        }
      }

      return completed;
    },
  );

  return { completedMetrics, seenMissingMetricKeys };
}

export function scoreRunV1(args: ScoreRunV1Args): ScoreResult {
  const decommissionedChains = new Set(args.decommissionedChains ?? []);
  const scoreableChains = args.allowedChains.filter(
    (chain) => !decommissionedChains.has(chain),
  );

  if (scoreableChains.length === 0) {
    throw new Error("scoreRunV1 requires at least one allowed chain");
  }

  const { completedMetrics, seenMissingMetricKeys } = buildCompletedMetrics({
    scenario: args.scenario,
    allowedChains: scoreableChains,
    chainMetrics: args.chainMetrics.filter(
      (metric) => !decommissionedChains.has(metric.chain),
    ),
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
    const components: RankedChain["components"] = {
      fee: 0,
      latency: 0,
      reliability: 0,
      liquidity: 0,
    };

    for (const metricKey of SCORING_V1_INPUT_METRIC_KEYS) {
      const { min, max } = bounds[metricKey];
      const range = max - min;

      const normalizedValue =
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
      const weight = SCORING_V1_WEIGHTS[componentKey];
      components[componentKey] = roundScore(normalizedValue * weight);
    }

    const weightedScore = roundScore(
      components.fee +
        components.latency +
        components.reliability +
        components.liquidity,
    );

    return {
      chain: metricRow.chain,
      score: weightedScore,
      components,
      raw: {
        feeRaw: metricRow.feeRaw,
        latencyRaw: metricRow.latencyRaw,
        reliabilityRaw: metricRow.reliabilityRaw,
        liquidityRaw: metricRow.liquidityRaw,
      },
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

  const topChain = rankedScoring[0];
  if (!topChain) {
    throw new Error("scoreRunV1 requires at least one scoreable chain");
  }

  const reasonCodes: ScoringV1ReasonCode[] = [
    scenarioReasonCodeByScenario[args.scenario],
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

  const topScoreGroup = rankedScoring.filter((chainScore) =>
    isTie(chainScore.score, topChain.score),
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
    runId: args.runId,
    customerId: args.customerId,
    ranked: rankedScoring.map((chainScore) => ({
      chain: chainScore.chain,
      score: chainScore.score,
      components: chainScore.components,
    })),
    recommendedChain: topChain.chain,
    confidence: topChain.score,
    reasonCodes,
  };
}

export function classifyActivationByThreshold(args: {
  threshold: number;
  ranked: RankedChain[];
  decommissionedChains?: string[];
}): ActivationDecision {
  if (args.threshold < 0 || args.threshold > 1) {
    throw new Error("activation threshold must be between 0 and 1");
  }

  const activeChains: string[] = [];
  const inactiveChains: string[] = [];
  const decommissionedChains = new Set(args.decommissionedChains ?? []);

  for (const entry of args.ranked) {
    if (decommissionedChains.has(entry.chain)) {
      continue;
    }
    if (entry.score >= args.threshold) {
      activeChains.push(entry.chain);
    } else {
      inactiveChains.push(entry.chain);
    }
  }

  return {
    thresholdUsed: args.threshold,
    activeChains,
    inactiveChains,
  };
}
