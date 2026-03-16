import type { RunStatus } from "@repo/shared-types";

export const RUN_TERMINAL_STATUSES = [
  "ccipConfirmed",
  "partialFailure",
  "skippedOverlap",
] as const;

type TerminalRunStatus = (typeof RUN_TERMINAL_STATUSES)[number];

type TransitionLogEvent =
  | "run.transition.accepted"
  | "run.transition.rejected"
  | "run.overlap.skipped"
  | "run.start.accepted";

export type RunTransitionLog = {
  level: "info" | "warn";
  event: TransitionLogEvent;
  runId: string;
  customerId: string;
  timestamp: string;
  fromStatus: RunStatus | null;
  toStatus: RunStatus;
  errorCode: string | null;
  errorMessage: string | null;
};

export type RunTransitionAccepted = {
  accepted: true;
  fromStatus: RunStatus;
  toStatus: RunStatus;
  log: RunTransitionLog;
};

export type RunTransitionRejected = {
  accepted: false;
  fromStatus: RunStatus;
  toStatus: RunStatus;
  errorCode: string;
  errorMessage: string;
  log: RunTransitionLog;
};

export type RunTransitionGuardResult =
  | RunTransitionAccepted
  | RunTransitionRejected;

export type RunStartDecision = {
  status: "started" | "skippedOverlap";
  log: RunTransitionLog;
};

type GuardRunStatusTransitionArgs = {
  runId: string;
  customerId: string;
  fromStatus: RunStatus;
  toStatus: RunStatus;
  timestamp?: string;
};

type DecideRunStartArgs = {
  runId: string;
  customerId: string;
  hasActiveRun: boolean;
  timestamp?: string;
};

const RUN_ALLOWED_TRANSITIONS: Record<RunStatus, readonly RunStatus[]> = {
  started: ["scored", "partialFailure"],
  scored: ["ccipSent", "partialFailure"],
  ccipSent: ["ccipConfirmed", "partialFailure"],
  ccipConfirmed: [],
  partialFailure: [],
  skippedOverlap: [],
};

function isTerminalStatus(status: RunStatus): status is TerminalRunStatus {
  return RUN_TERMINAL_STATUSES.includes(status as TerminalRunStatus);
}

function buildLogEntry(args: {
  level: "info" | "warn";
  event: TransitionLogEvent;
  runId: string;
  customerId: string;
  timestamp: string;
  fromStatus: RunStatus | null;
  toStatus: RunStatus;
  errorCode?: string;
  errorMessage?: string;
}): RunTransitionLog {
  return {
    level: args.level,
    event: args.event,
    runId: args.runId,
    customerId: args.customerId,
    timestamp: args.timestamp,
    fromStatus: args.fromStatus,
    toStatus: args.toStatus,
    errorCode: args.errorCode ?? null,
    errorMessage: args.errorMessage ?? null,
  };
}

export function guardRunStatusTransition(
  args: GuardRunStatusTransitionArgs,
): RunTransitionGuardResult {
  const timestamp = args.timestamp ?? new Date().toISOString();

  if (isTerminalStatus(args.fromStatus)) {
    const errorCode = "RUN_STATUS_TERMINAL";
    const errorMessage = `cannot transition from terminal status '${args.fromStatus}'`;

    return {
      accepted: false,
      fromStatus: args.fromStatus,
      toStatus: args.toStatus,
      errorCode,
      errorMessage,
      log: buildLogEntry({
        level: "warn",
        event: "run.transition.rejected",
        runId: args.runId,
        customerId: args.customerId,
        timestamp,
        fromStatus: args.fromStatus,
        toStatus: args.toStatus,
        errorCode,
        errorMessage,
      }),
    };
  }

  const allowedStatuses = RUN_ALLOWED_TRANSITIONS[args.fromStatus];
  if (!allowedStatuses.includes(args.toStatus)) {
    const errorCode = "RUN_STATUS_TRANSITION_INVALID";
    const errorMessage = `transition '${args.fromStatus}' -> '${args.toStatus}' is not allowed`;

    return {
      accepted: false,
      fromStatus: args.fromStatus,
      toStatus: args.toStatus,
      errorCode,
      errorMessage,
      log: buildLogEntry({
        level: "warn",
        event: "run.transition.rejected",
        runId: args.runId,
        customerId: args.customerId,
        timestamp,
        fromStatus: args.fromStatus,
        toStatus: args.toStatus,
        errorCode,
        errorMessage,
      }),
    };
  }

  return {
    accepted: true,
    fromStatus: args.fromStatus,
    toStatus: args.toStatus,
    log: buildLogEntry({
      level: "info",
      event: "run.transition.accepted",
      runId: args.runId,
      customerId: args.customerId,
      timestamp,
      fromStatus: args.fromStatus,
      toStatus: args.toStatus,
    }),
  };
}

export function decideRunStart(args: DecideRunStartArgs): RunStartDecision {
  const timestamp = args.timestamp ?? new Date().toISOString();

  if (args.hasActiveRun) {
    return {
      status: "skippedOverlap",
      log: buildLogEntry({
        level: "info",
        event: "run.overlap.skipped",
        runId: args.runId,
        customerId: args.customerId,
        timestamp,
        fromStatus: null,
        toStatus: "skippedOverlap",
      }),
    };
  }

  return {
    status: "started",
    log: buildLogEntry({
      level: "info",
      event: "run.start.accepted",
      runId: args.runId,
      customerId: args.customerId,
      timestamp,
      fromStatus: null,
      toStatus: "started",
    }),
  };
}
