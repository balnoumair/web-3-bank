import * as z from "zod";
import { SCORING_V1_WEIGHTS } from "./scoring-schema";

export const CustomerConfigSchema = z.object({
    customerId: z.string(),
    allowedChains: z.array(z.string()),
    active: z.boolean(),
    activationThreshold: z.number().min(0).max(1),
});
export type CustomerConfig = z.infer<typeof CustomerConfigSchema>;

export const ActivationThresholdSchema = z.number().min(0).max(1);
export type ActivationThreshold = z.infer<typeof ActivationThresholdSchema>;

export const ScenarioNameSchema = z.enum(["normal", "congested"]);
export type ScenarioName = z.infer<typeof ScenarioNameSchema>;

export const ChainMetricsSchema = z.object({
    chain: z.string(),
    feeRaw: z.number(),
    latencyRaw: z.number(),
    reliabilityRaw: z.number(),
    liquidityRaw: z.number(),
});
export type ChainMetrics = z.infer<typeof ChainMetricsSchema>;

export const ScoreWeightsSchema = z.object({
    fee: z.number(),
    latency: z.number(),
    reliability: z.number(),
    liquidity: z.number(),
});
export type ScoreWeights = z.infer<typeof ScoreWeightsSchema>;

export const RankedChainSchema = z.object({
    chain: z.string(),
    score: z.number(),
    components: z.object({
        fee: z.number(),
        latency: z.number(),
        reliability: z.number(),
        liquidity: z.number(),
    }),
});
export type RankedChain = z.infer<typeof RankedChainSchema>;

export const ScoreResultSchema = z.object({
    runId: z.string(),
    customerId: z.string(),
    ranked: z.array(RankedChainSchema),
    recommendedChain: z.string(),
    confidence: z.number(),
    reasonCodes: z.array(z.string()),
});
export type ScoreResult = z.infer<typeof ScoreResultSchema>;

export const RouteUpdatedSchema = z.object({
    runId: z.string(),
    customerId: z.string(),
    recommendedChain: z.string(),
    score: z.number(),
    timestamp: z.string(),
});
export type RouteUpdated = z.infer<typeof RouteUpdatedSchema>;

export const ActivationDecisionSchema = z.object({
    thresholdUsed: ActivationThresholdSchema,
    activeChains: z.array(z.string()),
    inactiveChains: z.array(z.string()),
});
export type ActivationDecision = z.infer<typeof ActivationDecisionSchema>;

export const RunStatusSchema = z.enum([
    "started",
    "scored",
    "ccipSent",
    "ccipConfirmed",
    "partialFailure",
    "skippedOverlap",
]);
export type RunStatus = z.infer<typeof RunStatusSchema>;

export const DEFAULT_SCORE_WEIGHTS: ScoreWeights = {
    ...SCORING_V1_WEIGHTS,
};
