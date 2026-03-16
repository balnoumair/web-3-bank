import { describe, expect, it } from "vitest";

import {
  CRE_1_HTTP_STATUS_BY_ERROR_CODE,
  toCrePolicyErrorResponse,
} from "../src";

describe("CRE Policy intake error mapping", () => {
  it("maps every ADR3 error code to deterministic HTTP status", () => {
    expect(CRE_1_HTTP_STATUS_BY_ERROR_CODE).toEqual({
      UNAUTHORIZED: 401,
      INVALID_PAYLOAD: 400,
      UNSUPPORTED_CHAIN: 400,
      INVALID_SCENARIO: 400,
      CUSTOMER_NOT_FOUND: 404,
      STORE_UNAVAILABLE: 503,
    });
  });

  it("keeps requestId in error body when available", () => {
    const response = toCrePolicyErrorResponse({
      errorCode: "UNSUPPORTED_CHAIN",
      errorMessage: "unsupported chain 'polygon-amoy'",
      requestId: "req-32-001",
    });

    expect(response).toEqual({
      status: 400,
      body: {
        errorCode: "UNSUPPORTED_CHAIN",
        errorMessage: "unsupported chain 'polygon-amoy'",
        requestId: "req-32-001",
      },
    });
  });

  it("sets requestId to null when unavailable", () => {
    const response = toCrePolicyErrorResponse({
      errorCode: "CUSTOMER_NOT_FOUND",
      errorMessage: "customer not found",
      requestId: null,
    });

    expect(response).toEqual({
      status: 404,
      body: {
        errorCode: "CUSTOMER_NOT_FOUND",
        errorMessage: "customer not found",
        requestId: null,
      },
    });
  });
});
