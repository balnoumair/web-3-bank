import type { RunStatus } from "@repo/shared-types";

export type TransitionCase = {
  id: string;
  fromStatus: RunStatus;
  toStatus: RunStatus;
  accepted: boolean;
  errorCode?: string;
};

export const VALID_TRANSITION_CASES: TransitionCase[] = [
  {
    id: "started-to-scored",
    fromStatus: "started",
    toStatus: "scored",
    accepted: true,
  },
  {
    id: "scored-to-ccip-sent",
    fromStatus: "scored",
    toStatus: "ccipSent",
    accepted: true,
  },
  {
    id: "ccip-sent-to-ccip-confirmed",
    fromStatus: "ccipSent",
    toStatus: "ccipConfirmed",
    accepted: true,
  },
  {
    id: "started-to-partial-failure",
    fromStatus: "started",
    toStatus: "partialFailure",
    accepted: true,
  },
  {
    id: "scored-to-partial-failure",
    fromStatus: "scored",
    toStatus: "partialFailure",
    accepted: true,
  },
  {
    id: "ccip-sent-to-partial-failure",
    fromStatus: "ccipSent",
    toStatus: "partialFailure",
    accepted: true,
  },
];

export const INVALID_TRANSITION_CASES: TransitionCase[] = [
  {
    id: "started-to-ccip-sent",
    fromStatus: "started",
    toStatus: "ccipSent",
    accepted: false,
    errorCode: "RUN_STATUS_TRANSITION_INVALID",
  },
  {
    id: "scored-to-ccip-confirmed",
    fromStatus: "scored",
    toStatus: "ccipConfirmed",
    accepted: false,
    errorCode: "RUN_STATUS_TRANSITION_INVALID",
  },
  {
    id: "partial-failure-to-scored",
    fromStatus: "partialFailure",
    toStatus: "scored",
    accepted: false,
    errorCode: "RUN_STATUS_TERMINAL",
  },
  {
    id: "ccip-confirmed-to-partial-failure",
    fromStatus: "ccipConfirmed",
    toStatus: "partialFailure",
    accepted: false,
    errorCode: "RUN_STATUS_TERMINAL",
  },
  {
    id: "skipped-overlap-to-started",
    fromStatus: "skippedOverlap",
    toStatus: "started",
    accepted: false,
    errorCode: "RUN_STATUS_TERMINAL",
  },
];
