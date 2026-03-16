import type {
  ScoreResult,
  RouteUpdatedOnchain,
  OnchainPublishResult,
  OnchainPublishPort,
  SimulationRequest,
  TransactionSimulationPort,
} from "@repo/shared-types";

// ── Score scaling constant ──────────────────────────────────────────

/** Multiplier to convert a 0–1 float score to an integer for Solidity uint256. */
export const SCORE_SCALE_FACTOR = 100;

// ── Score-to-onchain mapper ─────────────────────────────────────────

/**
 * Maps a ScoreResult (float score, no timestamp) to a RouteUpdatedOnchain
 * payload suitable for the RouteReceiver contract.
 *
 * - score is scaled to an integer: Math.floor(floatScore * 100)
 * - timestamp is provided as Unix epoch seconds (caller supplies it so
 *   the mapping is deterministic and testable).
 */
export function toRouteUpdatedOnchain(
  result: ScoreResult,
  epochSeconds: number,
): RouteUpdatedOnchain {
  const rankedMatch = result.ranked.find(
    (entry) => entry.chain === result.recommendedChain,
  );

  return {
    runId: result.runId,
    customerId: result.customerId,
    recommendedChain: result.recommendedChain,
    score: Math.floor((rankedMatch?.score ?? 0) * SCORE_SCALE_FACTOR),
    timestamp: epochSeconds,
  };
}

// ── Contract call abstraction (injectable dependency) ───────────────

export type PublishRouteTxReceipt = {
  txHash: string;
  blockNumber: number;
  logs: Array<{
    eventName: string;
    logIndex: number;
    args: {
      runId: string;
      customerId: string;
      recommendedChain: string;
      score: bigint;
      timestamp: bigint;
    };
  }>;
};

type OnchainPublishDeps = {
  /**
   * Calls RouteReceiver.publishRoute(...) and returns the tx receipt.
   * The adapter does not know about providers or signers — callers
   * wire those in when constructing deps.
   */
  publishRoute: (payload: RouteUpdatedOnchain) => Promise<PublishRouteTxReceipt>;
  simulationPort?: TransactionSimulationPort;
  buildSimulationRequest?: (payload: RouteUpdatedOnchain) => SimulationRequest;
};

// ── Adapter factory ─────────────────────────────────────────────────

const DUPLICATE_RUN_PATTERN = /runId already published/i;

/**
 * Creates an OnchainPublishPort backed by an injectable contract call.
 * Follows the same dependency-injection pattern as ccip-publisher.ts.
 */
export function createOnchainPublishAdapter(
  deps: OnchainPublishDeps,
): OnchainPublishPort {
  return {
    async publish(payload: RouteUpdatedOnchain): Promise<OnchainPublishResult> {
      try {
        let simulationId: string | undefined;
        let simulationTraceUrl: string | undefined;

        if (deps.simulationPort) {
          if (!deps.buildSimulationRequest) {
            return {
              status: "failed",
              error: "Simulation requested but buildSimulationRequest is missing",
            };
          }

          const simulationRequest = deps.buildSimulationRequest(payload);
          const simulationResult =
            await deps.simulationPort.simulate(simulationRequest);

          if (!simulationResult.success) {
            return {
              status: "failed",
              error:
                simulationResult.errorMessage ??
                "Simulation predicted transaction failure",
            };
          }

          simulationId = simulationResult.simulationId ?? undefined;
          simulationTraceUrl = simulationResult.traceUrl ?? undefined;
        }

        const receipt = await deps.publishRoute(payload);

        const routePublishedLog = receipt.logs.find(
          (log) => log.eventName === "RoutePublished",
        );

        if (!routePublishedLog) {
          return {
            status: "failed",
            error: "RoutePublished event not found in transaction logs",
          };
        }

        const confirmedResult: OnchainPublishResult = {
          status: "confirmed",
          txHash: receipt.txHash,
          blockNumber: receipt.blockNumber,
          eventLogIndex: routePublishedLog.logIndex,
          ...(simulationId ? { simulationId } : {}),
          ...(simulationTraceUrl ? { simulationTraceUrl } : {}),
        };

        return confirmedResult;
      } catch (error) {
        const message =
          error instanceof Error ? error.message : "unknown publish error";

        if (DUPLICATE_RUN_PATTERN.test(message)) {
          return {
            status: "duplicateRunId",
            runId: payload.runId,
          };
        }

        return {
          status: "failed",
          error: message,
        };
      }
    },
  };
}
