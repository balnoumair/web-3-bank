import type { RouteUpdated } from "@repo/shared-types";

export const CCIP_SEND_BACKOFF_MS = [1000, 2000] as const;
export const CCIP_MAX_SEND_ATTEMPTS = 3;

export type RouteUpdatedEnvelope = {
  sourceChain: "ethereum-sepolia";
  destinationChain: "base-sepolia";
  payload: RouteUpdated;
};

type PublishArgs = {
  routeUpdated: RouteUpdated;
  send: (args: {
    envelope: RouteUpdatedEnvelope;
    attempt: number;
  }) => Promise<{ ccipMessageId: string }>;
  idempotencyStore?: Set<string>;
  onBackoff?: (delayMs: number, attempt: number) => Promise<void> | void;
};

type PublishSuccess = {
  ok: true;
  runId: string;
  status: "ccipSent";
  attemptCount: number;
  ccipMessageId: string;
  envelope: RouteUpdatedEnvelope;
};

type PublishFailure = {
  ok: false;
  runId: string;
  status: "partialFailure";
  attemptCount: number;
  errorCode: "CCIP_SEND_RETRIES_EXHAUSTED";
  errorMessage: string;
};

type PublishDuplicate = {
  ok: false;
  runId: string;
  status: "duplicateBlocked";
  attemptCount: 0;
  errorCode: "CCIP_DUPLICATE_RUN_ID";
  errorMessage: string;
};

export type PublishRouteUpdateResult =
  | PublishSuccess
  | PublishFailure
  | PublishDuplicate;

function buildRouteUpdatedEnvelope(payload: RouteUpdated): RouteUpdatedEnvelope {
  return {
    sourceChain: "ethereum-sepolia",
    destinationChain: "base-sepolia",
    payload,
  };
}

export async function publishRouteUpdateWithRetry(
  args: PublishArgs,
): Promise<PublishRouteUpdateResult> {
  const idempotencyStore = args.idempotencyStore ?? new Set<string>();
  const runId = args.routeUpdated.runId;

  if (idempotencyStore.has(runId)) {
    return {
      ok: false,
      runId,
      status: "duplicateBlocked",
      attemptCount: 0,
      errorCode: "CCIP_DUPLICATE_RUN_ID",
      errorMessage: `duplicate publish blocked for runId '${runId}'`,
    };
  }

  const envelope = buildRouteUpdatedEnvelope(args.routeUpdated);
  let lastErrorMessage = "unknown ccip send error";

  for (let attempt = 1; attempt <= CCIP_MAX_SEND_ATTEMPTS; attempt += 1) {
    try {
      const sendResult = await args.send({ envelope, attempt });
      idempotencyStore.add(runId);

      return {
        ok: true,
        runId,
        status: "ccipSent",
        attemptCount: attempt,
        ccipMessageId: sendResult.ccipMessageId,
        envelope,
      };
    } catch (error) {
      if (error instanceof Error && error.message.length > 0) {
        lastErrorMessage = error.message;
      }

      if (attempt < CCIP_MAX_SEND_ATTEMPTS) {
        const backoffDelay = CCIP_SEND_BACKOFF_MS[attempt - 1] ?? 0;
        await args.onBackoff?.(backoffDelay, attempt);
      }
    }
  }

  return {
    ok: false,
    runId,
    status: "partialFailure",
    attemptCount: CCIP_MAX_SEND_ATTEMPTS,
    errorCode: "CCIP_SEND_RETRIES_EXHAUSTED",
    errorMessage: lastErrorMessage,
  };
}
