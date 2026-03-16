import * as z from "zod";

export const ScoringV1WeightsSchema = z.object({
  fee: z.literal(0.35),
  latency: z.literal(0.3),
  reliability: z.literal(0.25),
  liquidity: z.literal(0.1),
});

export const SCORING_V1_WEIGHTS = {
  fee: 0.35,
  latency: 0.3,
  reliability: 0.25,
  liquidity: 0.1,
} as const;

export const ScoringV1InputMetricKeySchema = z.enum([
  "feeRaw",
  "latencyRaw",
  "reliabilityRaw",
  "liquidityRaw",
]);

export type ScoringV1InputMetricKey = z.infer<
  typeof ScoringV1InputMetricKeySchema
>;

export const SCORING_V1_INPUT_METRIC_KEYS =
  ScoringV1InputMetricKeySchema.options;

export const ScoringV1ChainMetricsInputSchema = z.object({
  chain: z.string(),
  feeRaw: z.number().nullable().optional(),
  latencyRaw: z.number().nullable().optional(),
  reliabilityRaw: z.number().nullable().optional(),
  liquidityRaw: z.number().nullable().optional(),
});

export type ScoringV1ChainMetricsInput = z.infer<
  typeof ScoringV1ChainMetricsInputSchema
>;

export const SCORING_V1_METRIC_DIRECTION: Record<
  ScoringV1InputMetricKey,
  "lower-better" | "higher-better"
> = {
  feeRaw: "lower-better",
  latencyRaw: "lower-better",
  reliabilityRaw: "higher-better",
  liquidityRaw: "higher-better",
};

export const SCORING_V1_SCENARIO_FALLBACKS = {
  normal: {
    feeRaw: 35,
    latencyRaw: 220,
    reliabilityRaw: 0.965,
    liquidityRaw: 0.75,
  },
  congested: {
    feeRaw: 85,
    latencyRaw: 480,
    reliabilityRaw: 0.9,
    liquidityRaw: 0.55,
  },
} as const;

export const SCORING_V1_SCORE_TIE_EPSILON = 1e-9;

export const SCORING_V1_SCORE_DECIMALS = 6;

export const SCORING_V1_TIE_BREAK_ORDER = [
  "reliabilityRawDesc",
  "latencyRawAsc",
  "chainLexAsc",
] as const;

export const ScoringV1ReasonCodeSchema = z.enum([
  "SCENARIO_NORMAL",
  "SCENARIO_CONGESTED",
  "MISSING_FEE_RAW_FALLBACK",
  "MISSING_LATENCY_RAW_FALLBACK",
  "MISSING_RELIABILITY_RAW_FALLBACK",
  "MISSING_LIQUIDITY_RAW_FALLBACK",
  "ZERO_RANGE_FEE_RAW",
  "ZERO_RANGE_LATENCY_RAW",
  "ZERO_RANGE_RELIABILITY_RAW",
  "ZERO_RANGE_LIQUIDITY_RAW",
  "TIE_BREAK_RELIABILITY",
  "TIE_BREAK_LATENCY",
  "TIE_BREAK_CHAIN",
]);

export type ScoringV1ReasonCode = z.infer<typeof ScoringV1ReasonCodeSchema>;

export const SCORING_V1_REASON_CODES = ScoringV1ReasonCodeSchema.options;
