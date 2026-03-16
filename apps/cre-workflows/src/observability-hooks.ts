import type {
  CcipDeliveryRecord,
  CcipDeliveryStatus,
  ScenarioName,
  ScoringRunRecord,
  ScoringV1ReasonCode,
} from "@repo/shared-types";
import type { RunStatus } from "@repo/shared-types";

import { guardRunStatusTransition } from "./run-state-machine";

type TracePoint = "started" | "scored" | "ccipSent" | "terminal";

export type RunTraceEvent = {
  tracePoint: TracePoint;
  runId: string;
  customerId: string;
  requestId: string | null;
  configVersion: number | null;
  snapshotUpdatedAt: string | null;
  status: RunStatus;
  timestamp: string;
  reasonCodes: ScoringV1ReasonCode[];
  errorCode: string | null;
  errorMessage: string | null;
  attemptCount: number;
  ccipMessageId: string | null;
};

type CommonRunArgs = {
  runId: string;
  customerId: string;
  requestId?: string;
  configVersion?: number;
  snapshotUpdatedAt?: string;
  scenario: ScenarioName;
  recommendedChain: string | null;
  score: number | null;
  reasonCodes: ScoringV1ReasonCode[];
  attemptCount: number;
  timestamp: string;
  errorCode?: string;
  errorMessage?: string;
};

type StartedArgs = {
  runId: string;
  customerId: string;
  requestId?: string;
  configVersion?: number;
  snapshotUpdatedAt?: string;
  scenario: ScenarioName;
  timestamp: string;
};

type ScoredArgs = {
  previousStatus: RunStatus;
} & CommonRunArgs & {
    recommendedChain: string;
    score: number;
  };

type CcipSentArgs = {
  previousStatus: RunStatus;
  deliveryId: string;
  ccipMessageId: string;
} & CommonRunArgs & {
    recommendedChain: string;
    score: number;
  };

type TerminalArgs = {
  previousStatus: RunStatus;
  terminalStatus: "ccipConfirmed" | "partialFailure" | "skippedOverlap";
  deliveryId?: string;
  ccipMessageId?: string;
} & CommonRunArgs;

type CreateObservabilityHooksArgs = {
  log: (event: RunTraceEvent) => void;
  saveRun: (record: ScoringRunRecord) => void;
  saveDelivery: (record: CcipDeliveryRecord) => void;
};

function requireValidTransition(args: {
  runId: string;
  customerId: string;
  fromStatus: RunStatus;
  toStatus: RunStatus;
  timestamp: string;
}): void {
  const transition = guardRunStatusTransition(args);
  if (!transition.accepted) {
    throw new Error(`${transition.errorCode}: ${transition.errorMessage}`);
  }
}

function createRunRecord(args: CommonRunArgs & { status: RunStatus }): ScoringRunRecord {
  return {
    runId: args.runId,
    customerId: args.customerId,
    requestId: args.requestId ?? null,
    configVersion: args.configVersion ?? null,
    snapshotUpdatedAt: args.snapshotUpdatedAt ?? null,
    status: args.status,
    timestamp: args.timestamp,
    scenario: args.scenario,
    recommendedChain: args.recommendedChain,
    score: args.score,
    reasonCodes: args.reasonCodes,
    attemptCount: args.attemptCount,
    errorCode: args.errorCode ?? null,
    errorMessage: args.errorMessage ?? null,
  };
}

function createDeliveryRecord(args: {
  runId: string;
  deliveryId: string;
  ccipMessageId: string | null;
  status: CcipDeliveryStatus;
  attemptCount: number;
  timestamp: string;
  errorCode?: string;
  errorMessage?: string;
}): CcipDeliveryRecord {
  return {
    deliveryId: args.deliveryId,
    runId: args.runId,
    ccipMessageId: args.ccipMessageId,
    sourceChain: "ethereum-sepolia",
    destinationChain: "base-sepolia",
    status: args.status,
    attemptCount: args.attemptCount,
    errorCode: args.errorCode ?? null,
    errorMessage: args.errorMessage ?? null,
    timestamp: args.timestamp,
  };
}

function emitTrace(args: {
  sink: (event: RunTraceEvent) => void;
  tracePoint: TracePoint;
  runRecord: ScoringRunRecord;
  ccipMessageId?: string | null;
}): void {
  args.sink({
    tracePoint: args.tracePoint,
    runId: args.runRecord.runId,
    customerId: args.runRecord.customerId,
    requestId: args.runRecord.requestId,
    configVersion: args.runRecord.configVersion,
    snapshotUpdatedAt: args.runRecord.snapshotUpdatedAt,
    status: args.runRecord.status,
    timestamp: args.runRecord.timestamp,
    reasonCodes: args.runRecord.reasonCodes,
    errorCode: args.runRecord.errorCode,
    errorMessage: args.runRecord.errorMessage,
    attemptCount: args.runRecord.attemptCount,
    ccipMessageId: args.ccipMessageId ?? null,
  });
}

export function createObservabilityHooks(args: CreateObservabilityHooksArgs): {
  onStarted: (input: StartedArgs) => ScoringRunRecord;
  onScored: (input: ScoredArgs) => ScoringRunRecord;
  onCcipSent: (input: CcipSentArgs) => {
    runRecord: ScoringRunRecord;
    deliveryRecord: CcipDeliveryRecord;
  };
  onTerminal: (input: TerminalArgs) => {
    runRecord: ScoringRunRecord;
    deliveryRecord: CcipDeliveryRecord | null;
  };
} {
  return {
    onStarted: (input) => {
      const runRecord = createRunRecord({
        runId: input.runId,
        customerId: input.customerId,
        requestId: input.requestId,
        configVersion: input.configVersion,
        snapshotUpdatedAt: input.snapshotUpdatedAt,
        status: "started",
        timestamp: input.timestamp,
        scenario: input.scenario,
        recommendedChain: null,
        score: null,
        reasonCodes: [],
        attemptCount: 0,
      });

      emitTrace({ sink: args.log, tracePoint: "started", runRecord });
      args.saveRun(runRecord);
      return runRecord;
    },

    onScored: (input) => {
      requireValidTransition({
        runId: input.runId,
        customerId: input.customerId,
        fromStatus: input.previousStatus,
        toStatus: "scored",
        timestamp: input.timestamp,
      });

      const runRecord = createRunRecord({
        ...input,
        status: "scored",
      });

      emitTrace({ sink: args.log, tracePoint: "scored", runRecord });
      args.saveRun(runRecord);
      return runRecord;
    },

    onCcipSent: (input) => {
      requireValidTransition({
        runId: input.runId,
        customerId: input.customerId,
        fromStatus: input.previousStatus,
        toStatus: "ccipSent",
        timestamp: input.timestamp,
      });

      const runRecord = createRunRecord({
        ...input,
        status: "ccipSent",
      });
      const deliveryRecord = createDeliveryRecord({
        runId: input.runId,
        deliveryId: input.deliveryId,
        ccipMessageId: input.ccipMessageId,
        status: "sent",
        attemptCount: input.attemptCount,
        timestamp: input.timestamp,
      });

      emitTrace({
        sink: args.log,
        tracePoint: "ccipSent",
        runRecord,
        ccipMessageId: input.ccipMessageId,
      });
      args.saveRun(runRecord);
      args.saveDelivery(deliveryRecord);

      return { runRecord, deliveryRecord };
    },

    onTerminal: (input) => {
      requireValidTransition({
        runId: input.runId,
        customerId: input.customerId,
        fromStatus: input.previousStatus,
        toStatus: input.terminalStatus,
        timestamp: input.timestamp,
      });

      const runRecord = createRunRecord({
        ...input,
        status: input.terminalStatus,
      });

      let deliveryRecord: CcipDeliveryRecord | null = null;
      if (input.deliveryId) {
        deliveryRecord = createDeliveryRecord({
          runId: input.runId,
          deliveryId: input.deliveryId,
          ccipMessageId: input.ccipMessageId ?? null,
          status: input.terminalStatus === "ccipConfirmed" ? "confirmed" : "failed",
          attemptCount: input.attemptCount,
          timestamp: input.timestamp,
          errorCode: input.errorCode,
          errorMessage: input.errorMessage,
        });
      }

      emitTrace({
        sink: args.log,
        tracePoint: "terminal",
        runRecord,
        ccipMessageId: input.ccipMessageId ?? null,
      });
      args.saveRun(runRecord);
      if (deliveryRecord) {
        args.saveDelivery(deliveryRecord);
      }

      return { runRecord, deliveryRecord };
    },
  };
}
