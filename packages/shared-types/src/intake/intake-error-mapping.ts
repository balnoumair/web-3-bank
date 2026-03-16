import type { CrePolicyErrorCode, CrePolicyErrorResponse } from "./cre-policy-api-contract";

export type CrePolicyErrorHttpStatus = 400 | 401 | 404 | 503;

export const CRE_1_HTTP_STATUS_BY_ERROR_CODE: Record<
  CrePolicyErrorCode,
  CrePolicyErrorHttpStatus
> = {
  UNAUTHORIZED: 401,
  INVALID_PAYLOAD: 400,
  UNSUPPORTED_CHAIN: 400,
  INVALID_SCENARIO: 400,
  CUSTOMER_NOT_FOUND: 404,
  STORE_UNAVAILABLE: 503,
};

export function toCrePolicyErrorResponse(args: {
  errorCode: CrePolicyErrorCode;
  errorMessage: string;
  requestId: string | null;
}): {
  status: CrePolicyErrorHttpStatus;
  body: CrePolicyErrorResponse;
} {
  return {
    status: CRE_1_HTTP_STATUS_BY_ERROR_CODE[args.errorCode],
    body: {
      errorCode: args.errorCode,
      errorMessage: args.errorMessage,
      requestId: args.requestId,
    },
  };
}

export function isStoreUnavailableError(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "errorCode" in error &&
    (error as { errorCode?: string }).errorCode === "STORE_UNAVAILABLE"
  );
}
