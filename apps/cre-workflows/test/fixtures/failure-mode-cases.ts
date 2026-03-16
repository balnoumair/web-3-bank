export type FailureModeCase = {
  id: string;
  expectedTerminalStatus: "partialFailure" | "skippedOverlap";
  expectedErrorCode: string | null;
};

export const FAILURE_MODE_CASES: FailureModeCase[] = [
  {
    id: "send-failure-after-retries",
    expectedTerminalStatus: "partialFailure",
    expectedErrorCode: "CCIP_SEND_RETRIES_EXHAUSTED",
  },
  {
    id: "confirmation-timeout",
    expectedTerminalStatus: "partialFailure",
    expectedErrorCode: "CCIP_CONFIRMATION_TIMEOUT",
  },
  {
    id: "overlap-skip",
    expectedTerminalStatus: "skippedOverlap",
    expectedErrorCode: null,
  },
];
