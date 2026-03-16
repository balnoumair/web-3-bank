import type { RouteUpdated, ScoreResult } from "@repo/shared-types";

export * from "./run-state-machine";
export * from "./scoring-engine";
export * from "./ccip-publisher";
export * from "./destination-confirmation";
export * from "./observability-hooks";
export * from "./runners/run-lifecycle-orchestrator";
export * from "./run-start-snapshot-contract";
export * from "./runners/config-to-run-binding-runner";
export * from "./runners/auth-handoff-runner";
export * from "./onchain-publish-adapter";

export * from "./workflow-utils";

export function toRouteUpdated(result: ScoreResult): RouteUpdated {
  const rankedMatch = result.ranked.find(
    (entry) => entry.chain === result.recommendedChain,
  );

  return {
    runId: result.runId,
    customerId: result.customerId,
    recommendedChain: result.recommendedChain,
    score: rankedMatch?.score ?? 0,
    timestamp: new Date().toISOString(),
  };
}
