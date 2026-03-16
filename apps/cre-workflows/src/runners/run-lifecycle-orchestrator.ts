import type {
  ActivationDecision,
  CcipDeliveryRecord,
  CrePolicyConfigSnapshot,
  RouteUpdated,
  ScenarioName,
  ScoreResult,
  ScoringRunRecord,
  ScoringV1ChainMetricsInput,
  ScoringV1ReasonCode,
} from "@repo/shared-types";
import { ScoringV1ReasonCodeSchema } from "@repo/shared-types";

import {
  createObservabilityHooks,
  type RunTraceEvent,
} from "../observability-hooks";
import {
  bindRunStartConfigSnapshot,
} from "../run-start-snapshot-contract";
import {
  handleDestinationConfirmation,
} from "../destination-confirmation";
import {
  publishRouteUpdateWithRetry,
} from "../ccip-publisher";
import {
  scoreRunV1,
  classifyActivationByThreshold,
} from "../scoring-engine";
import { addMilliseconds, buildSequenceId } from "../workflow-utils";
import { toRouteUpdated } from "../index";

type HappyPathValidationInput = {
  customerId: string;
  scenario: ScenarioName;
  activationThreshold?: number;
  allowedChains: string[];
  chainMetrics: ScoringV1ChainMetricsInput[];
};

type HappyPathValidationEvidence = {
  runId: string;
  timeline: ScoringRunRecord["status"][];
  runStartSnapshotVersion: number;
  scoreResult: ScoreResult;
  activationDecision: ActivationDecision;
  routeUpdated: RouteUpdated;
  ccipMessageId: string;
  runRecords: ScoringRunRecord[];
  deliveryRecords: CcipDeliveryRecord[];
  traceEvents: RunTraceEvent[];
};

type CreateHappyPathValidationRunnerArgs = {
  runIdPrefix?: string;
  baseTimestamp?: string;
  readConfigSnapshot?: (args: {
    customerId: string;
    scenario: ScenarioName;
    allowedChains: string[];
    timestamp: string;
  }) => Promise<CrePolicyConfigSnapshot | null>;
};

function buildMessageId(runId: string): string {
  return `0x${runId.replace(/-/g, "")}`;
}

function normalizeReasonCodes(reasonCodes: string[]): ScoringV1ReasonCode[] {
  return reasonCodes.map((reasonCode) => ScoringV1ReasonCodeSchema.parse(reasonCode));
}

export function createHappyPathValidationRunner(
  args: CreateHappyPathValidationRunnerArgs = {},
): {
  runValidation: (
    input: HappyPathValidationInput,
  ) => Promise<HappyPathValidationEvidence>;
} {
  const runIdPrefix = args.runIdPrefix ?? "run-happy-path";
  const baseTimestamp = args.baseTimestamp ?? "2026-02-21T18:00:00.000Z";
  const idempotencyStore = new Set<string>();
  let runSequence = 0;

  return {
    runValidation: async (input) => {
      runSequence += 1;
      const runId = buildSequenceId(runIdPrefix, runSequence);

      const runOffset = (runSequence - 1) * 60_000;
      const startedAt = addMilliseconds(baseTimestamp, runOffset);
      const scoredAt = addMilliseconds(startedAt, 1_000);
      const sentAt = addMilliseconds(scoredAt, 1_000);
      const confirmedAt = addMilliseconds(sentAt, 1_000);
      const timeoutAt = addMilliseconds(sentAt, 60_000);

      const runRecords: ScoringRunRecord[] = [];
      const deliveryRecords: CcipDeliveryRecord[] = [];
      const traceEvents: RunTraceEvent[] = [];
      const hooks = createObservabilityHooks({
        log: (event) => {
          traceEvents.push(event);
        },
        saveRun: (record) => {
          runRecords.push(record);
        },
        saveDelivery: (record) => {
          deliveryRecords.push(record);
        },
      });

      const snapshotBinding = await bindRunStartConfigSnapshot({
        runId,
        customerId: input.customerId,
        hasActiveRun: false,
        timestamp: startedAt,
        readConfigSnapshot: async () => {
          if (args.readConfigSnapshot) {
            return args.readConfigSnapshot({
              customerId: input.customerId,
              scenario: input.scenario,
              allowedChains: input.allowedChains,
              timestamp: startedAt,
            });
          }

          return {
            customerId: input.customerId,
            configVersion: 1,
            updatedAt: startedAt,
            allowedChains: [...input.allowedChains],
            active: true,
            scenario: input.scenario,
            activationThreshold: input.activationThreshold ?? 0.7,
          };
        },
      });
      if (snapshotBinding.status !== "started") {
        throw new Error(`unexpected start decision status '${snapshotBinding.status}'`);
      }

      hooks.onStarted({
        runId,
        customerId: input.customerId,
        scenario: snapshotBinding.snapshot.scenario,
        timestamp: startedAt,
      });

      const scoreResult = scoreRunV1({
        runId,
        customerId: input.customerId,
        scenario: snapshotBinding.snapshot.scenario,
        allowedChains: snapshotBinding.snapshot.allowedChains,
        chainMetrics: input.chainMetrics,
      });

      const recommendedEntry = scoreResult.ranked.find(
        (entry) => entry.chain === scoreResult.recommendedChain,
      );
      if (!recommendedEntry) {
        throw new Error("recommended chain not found in ranked scoring output");
      }

      const reasonCodes = normalizeReasonCodes(scoreResult.reasonCodes);
      const activationDecision = classifyActivationByThreshold({
        threshold: input.activationThreshold ?? 0.7,
        ranked: scoreResult.ranked,
      });

      hooks.onScored({
        runId,
        customerId: input.customerId,
        scenario: snapshotBinding.snapshot.scenario,
        previousStatus: "started",
        recommendedChain: scoreResult.recommendedChain,
        score: recommendedEntry.score,
        reasonCodes,
        attemptCount: 0,
        timestamp: scoredAt,
      });

      const routeUpdated = toRouteUpdated(scoreResult);
      const ccipMessageId = buildMessageId(runId);

      const publishResult = await publishRouteUpdateWithRetry({
        routeUpdated,
        idempotencyStore,
        send: async () => ({ ccipMessageId }),
      });

      if (!publishResult.ok) {
        throw new Error(
          `happy path publish failed: ${publishResult.errorCode} ${publishResult.errorMessage}`,
        );
      }

      const deliveryId = `delivery-${runId}`;

      hooks.onCcipSent({
        runId,
        customerId: input.customerId,
        scenario: snapshotBinding.snapshot.scenario,
        previousStatus: "scored",
        deliveryId,
        recommendedChain: scoreResult.recommendedChain,
        score: recommendedEntry.score,
        reasonCodes,
        attemptCount: publishResult.attemptCount,
        ccipMessageId,
        timestamp: sentAt,
      });

      const confirmationResult = handleDestinationConfirmation({
        runId,
        customerId: input.customerId,
        currentStatus: "ccipSent",
        ccipMessageId,
        confirmationTimeoutAt: timeoutAt,
        now: confirmedAt,
        confirmationEvent: {
          runId,
          ccipMessageId,
          destinationTransactionHash: `0xconfirm${runId.replace(/-/g, "")}`,
          observedAt: confirmedAt,
        },
      });

      if (confirmationResult.status !== "ccipConfirmed") {
        throw new Error(
          `happy path confirmation failed with status '${confirmationResult.status}'`,
        );
      }

      hooks.onTerminal({
        runId,
        customerId: input.customerId,
        scenario: snapshotBinding.snapshot.scenario,
        previousStatus: "ccipSent",
        terminalStatus: "ccipConfirmed",
        recommendedChain: scoreResult.recommendedChain,
        score: recommendedEntry.score,
        reasonCodes,
        attemptCount: publishResult.attemptCount,
        deliveryId,
        ccipMessageId,
        timestamp: confirmedAt,
      });

      return {
        runId,
        timeline: runRecords.map((record) => record.status),
        runStartSnapshotVersion: snapshotBinding.log.snapshotVersion,
        scoreResult,
        activationDecision,
        routeUpdated,
        ccipMessageId,
        runRecords,
        deliveryRecords,
        traceEvents,
      };
    },
  };
}
