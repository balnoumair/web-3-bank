import type { RunStatus } from "@repo/shared-types";

import {
  guardRunStatusTransition,
  type RunTransitionAccepted,
  type RunTransitionGuardResult,
} from "./run-state-machine";

export const CCIP_CONFIRMATION_TIMEOUT_ERROR_CODE = "CCIP_CONFIRMATION_TIMEOUT";

type DestinationConfirmationEvent = {
  runId: string;
  ccipMessageId: string;
  destinationTransactionHash: string;
  observedAt: string;
};

type HandleDestinationConfirmationArgs = {
  runId: string;
  customerId: string;
  currentStatus: RunStatus;
  ccipMessageId: string;
  confirmationTimeoutAt: string;
  now?: string;
  confirmationEvent: DestinationConfirmationEvent | null;
};

type AwaitingConfirmation = {
  status: "awaitingConfirmation";
  runId: string;
  ccipMessageId: string;
  timeoutAt: string;
};

type ConfirmationRejected = {
  status: "rejected";
  runId: string;
  errorCode: "RUN_STATUS_TRANSITION_INVALID";
  errorMessage: "destination confirmation can only be handled from 'ccipSent'";
};

type ConfirmationTimedOut = {
  status: "partialFailure";
  runId: string;
  errorCode: typeof CCIP_CONFIRMATION_TIMEOUT_ERROR_CODE;
  errorMessage: string;
  transition: RunTransitionAccepted;
};

type ConfirmationCompleted = {
  status: "ccipConfirmed";
  runId: string;
  transition: RunTransitionAccepted;
  confirmation: DestinationConfirmationEvent;
};

export type DestinationConfirmationResult =
  | AwaitingConfirmation
  | ConfirmationRejected
  | ConfirmationTimedOut
  | ConfirmationCompleted;

function toEpochMs(timestamp: string): number {
  return new Date(timestamp).getTime();
}

function requireAcceptedTransition(
  transition: RunTransitionGuardResult,
): RunTransitionAccepted {
  if (!transition.accepted) {
    throw new Error(
      `unexpected rejected transition: ${transition.errorCode} ${transition.errorMessage}`,
    );
  }

  return transition;
}

export function handleDestinationConfirmation(
  args: HandleDestinationConfirmationArgs,
): DestinationConfirmationResult {
  if (args.currentStatus !== "ccipSent") {
    return {
      status: "rejected",
      runId: args.runId,
      errorCode: "RUN_STATUS_TRANSITION_INVALID",
      errorMessage: "destination confirmation can only be handled from 'ccipSent'",
    };
  }

  const now = args.now ?? new Date().toISOString();

  const confirmationEvent = args.confirmationEvent;
  if (
    confirmationEvent &&
    confirmationEvent.runId === args.runId &&
    confirmationEvent.ccipMessageId === args.ccipMessageId
  ) {
    const transition = requireAcceptedTransition(
      guardRunStatusTransition({
        runId: args.runId,
        customerId: args.customerId,
        fromStatus: "ccipSent",
        toStatus: "ccipConfirmed",
        timestamp: now,
      }),
    );

    return {
      status: "ccipConfirmed",
      runId: args.runId,
      transition,
      confirmation: confirmationEvent,
    };
  }

  if (toEpochMs(now) >= toEpochMs(args.confirmationTimeoutAt)) {
    const transition = requireAcceptedTransition(
      guardRunStatusTransition({
        runId: args.runId,
        customerId: args.customerId,
        fromStatus: "ccipSent",
        toStatus: "partialFailure",
        timestamp: now,
      }),
    );

    return {
      status: "partialFailure",
      runId: args.runId,
      errorCode: CCIP_CONFIRMATION_TIMEOUT_ERROR_CODE,
      errorMessage: `destination confirmation timed out for runId '${args.runId}'`,
      transition,
    };
  }

  return {
    status: "awaitingConfirmation",
    runId: args.runId,
    ccipMessageId: args.ccipMessageId,
    timeoutAt: args.confirmationTimeoutAt,
  };
}
