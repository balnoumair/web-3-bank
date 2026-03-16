import { describe, expect, it } from "vitest";

import {
  SCORE_WEIGHTS,
  ScoreWeightsSchema,
  SUPPORTED_SCENARIOS,
  SupportedScenarioSchema,
} from "../src";

describe("config package", () => {
  it("keeps scoring weights normalized", () => {
    const sum =
      SCORE_WEIGHTS.fee +
      SCORE_WEIGHTS.latency +
      SCORE_WEIGHTS.reliability +
      SCORE_WEIGHTS.liquidity;

    expect(sum).toBeCloseTo(1, 10);
  });

  it("defines supported scenarios", () => {
    expect(SUPPORTED_SCENARIOS).toEqual(["normal", "congested"]);
  });

  it("validates supported scenarios via schema", () => {
    expect(() => SupportedScenarioSchema.parse("normal")).not.toThrow();
    expect(() => SupportedScenarioSchema.parse("invalid")).toThrow();
  });

  it("validates score weights via schema", () => {
    expect(() => ScoreWeightsSchema.parse(SCORE_WEIGHTS)).not.toThrow();
  });
});
