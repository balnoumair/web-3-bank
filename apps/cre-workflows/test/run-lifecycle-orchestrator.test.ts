import { describe, expect, it } from "vitest";

import { createHappyPathValidationRunner } from "../src";
import { HAPPY_PATH_VALIDATION_CASE } from "./fixtures/run-lifecycle-orchestrator-case";

describe("CRE Evaluator happy path validation", () => {
  it("reaches started -> scored -> ccipSent -> ccipConfirmed", async () => {
    const runner = createHappyPathValidationRunner();

    const evidence = await runner.runValidation(HAPPY_PATH_VALIDATION_CASE);

    expect(evidence.timeline).toEqual([
      "started",
      "scored",
      "ccipSent",
      "ccipConfirmed",
    ]);
  });

  it("correlates output and delivery evidence by runId", async () => {
    const runner = createHappyPathValidationRunner();

    const evidence = await runner.runValidation(HAPPY_PATH_VALIDATION_CASE);

    expect(evidence.scoreResult.runId).toBe(evidence.runId);
    expect(evidence.routeUpdated.runId).toBe(evidence.runId);
    expect(evidence.runRecords.every((record) => record.runId === evidence.runId)).toBe(
      true,
    );
    expect(
      evidence.deliveryRecords.every((record) => record.runId === evidence.runId),
    ).toBe(true);
    expect(evidence.activationDecision.thresholdUsed).toBe(0.6);
    expect(evidence.activationDecision.activeChains.length).toBeGreaterThan(0);
  });

  it("can be rerun without manual data repair", async () => {
    const runner = createHappyPathValidationRunner({ runIdPrefix: "dev-21" });

    const first = await runner.runValidation(HAPPY_PATH_VALIDATION_CASE);
    const second = await runner.runValidation(HAPPY_PATH_VALIDATION_CASE);

    expect(first.runId).not.toBe(second.runId);
    expect(first.timeline.at(-1)).toBe("ccipConfirmed");
    expect(second.timeline.at(-1)).toBe("ccipConfirmed");
  });
});
