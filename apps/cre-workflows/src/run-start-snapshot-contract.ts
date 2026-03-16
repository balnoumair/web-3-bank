import type { CrePolicyConfigSnapshot } from "@repo/shared-types";

import { decideRunStart, type RunTransitionLog } from "./run-state-machine";

type RunStartSnapshotLog = RunTransitionLog & {
  snapshotVersion: number;
  snapshotUpdatedAt: string;
};

type RunStartSnapshotBindingSkipped = {
  status: "skippedOverlap";
  log: RunTransitionLog;
};

type RunStartSnapshotBindingStarted = {
  status: "started";
  snapshot: CrePolicyConfigSnapshot;
  log: RunStartSnapshotLog;
};

type RunStartSnapshotErrorCode = "CONFIG_NOT_FOUND" | "CONFIG_INACTIVE";

type RunStartSnapshotBindingPartialFailure = {
  status: "partialFailure";
  errorCode: RunStartSnapshotErrorCode;
  errorMessage: string;
  log: RunTransitionLog;
};

export type RunStartSnapshotBinding =
  | RunStartSnapshotBindingSkipped
  | RunStartSnapshotBindingStarted
  | RunStartSnapshotBindingPartialFailure;

type BindRunStartConfigSnapshotArgs = {
  runId: string;
  customerId: string;
  hasActiveRun: boolean;
  timestamp?: string;
  readConfigSnapshot: (customerId: string) => Promise<CrePolicyConfigSnapshot | null>;
};

function cloneSnapshot(snapshot: CrePolicyConfigSnapshot): CrePolicyConfigSnapshot {
  return {
    customerId: snapshot.customerId,
    configVersion: snapshot.configVersion,
    updatedAt: snapshot.updatedAt,
    allowedChains: [...snapshot.allowedChains],
    active: snapshot.active,
    scenario: snapshot.scenario,
    activationThreshold: snapshot.activationThreshold,
  };
}

function toPartialFailure(args: {
  decisionLog: RunTransitionLog;
  errorCode: RunStartSnapshotErrorCode;
  errorMessage: string;
}): RunStartSnapshotBindingPartialFailure {
  return {
    status: "partialFailure",
    errorCode: args.errorCode,
    errorMessage: args.errorMessage,
    log: {
      level: "warn",
      event: "run.transition.rejected",
      runId: args.decisionLog.runId,
      customerId: args.decisionLog.customerId,
      timestamp: args.decisionLog.timestamp,
      fromStatus: null,
      toStatus: "partialFailure",
      errorCode: args.errorCode,
      errorMessage: args.errorMessage,
    },
  };
}

export async function bindRunStartConfigSnapshot(
  args: BindRunStartConfigSnapshotArgs,
): Promise<RunStartSnapshotBinding> {
  const decision = decideRunStart({
    runId: args.runId,
    customerId: args.customerId,
    hasActiveRun: args.hasActiveRun,
    timestamp: args.timestamp,
  });

  if (decision.status !== "started") {
    return {
      status: "skippedOverlap",
      log: decision.log,
    };
  }

  const snapshot = await args.readConfigSnapshot(args.customerId);
  if (!snapshot) {
    return toPartialFailure({
      decisionLog: decision.log,
      errorCode: "CONFIG_NOT_FOUND",
      errorMessage: "config snapshot not found",
    });
  }

  if (!snapshot.active) {
    return toPartialFailure({
      decisionLog: decision.log,
      errorCode: "CONFIG_INACTIVE",
      errorMessage: "config snapshot is inactive",
    });
  }

  const frozenSnapshot = cloneSnapshot(snapshot);

  return {
    status: "started",
    snapshot: frozenSnapshot,
    log: {
      ...decision.log,
      snapshotVersion: frozenSnapshot.configVersion,
      snapshotUpdatedAt: frozenSnapshot.updatedAt,
    },
  };
}
