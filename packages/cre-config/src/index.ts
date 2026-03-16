import * as z from "zod";

export const SupportedScenarioSchema = z.enum(["normal", "congested"]);
export type SupportedScenario = z.infer<typeof SupportedScenarioSchema>;
export const SUPPORTED_SCENARIOS = SupportedScenarioSchema.options;

export const ScoreWeightsSchema = z.object({
  fee: z.number(),
  latency: z.number(),
  reliability: z.number(),
  liquidity: z.number(),
});
/*
 * Note: We do not export a type for ScoreWeights here to avoid conflict
 * if consumers use @repo/shared-types. However, if this package is standalone,
 * we might want to. Given the current file didn't export ScoreWeights type
 * (it only exported the value), we will keep it that way or export it if needed.
 * Actually, the previous file exported `SupportedScenario` type but NOT `ScoreWeights` type?
 * Let's check step 336.
 * It exported `SUPPORTED_SCENARIOS`, `SupportedScenario`, and `SCORE_WEIGHTS`.
 * It did NOT export `ScoreWeights` type explicitly.
 * So we will stick to that.
 */

export const SCORE_WEIGHTS = {
  fee: 0.35,
  latency: 0.3,
  reliability: 0.25,
  liquidity: 0.1,
} as const;
