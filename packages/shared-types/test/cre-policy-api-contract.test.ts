import { describe, expect, it } from "vitest";

import {
  ActivationDecisionSchema,
  CRE_POLICY_ERROR_CODES,
  CrePolicyConfigSnapshotSchema,
  CrePolicyConfigUpsertRequestSchema,
  CrePolicyErrorCodeSchema,
  CrePolicyErrorResponseSchema,
  CrePolicyGetConfigParamsSchema,
  CrePolicyScenarioUpdateRequestSchema,
  parseCrePolicyConfigUpsertRequest,
  parseCrePolicyScenarioUpdateRequest,
} from "../src";
import {
  VALID_CONFIG_SNAPSHOT,
  VALID_CONFIG_UPSERT_REQUEST,
  VALID_SCENARIO_UPDATE_REQUEST,
} from "./fixtures/cre-policy-api-cases";

describe("CRE Policy API contract", () => {
  it("accepts valid POST /config payloads", () => {
    const parsed = CrePolicyConfigUpsertRequestSchema.parse(VALID_CONFIG_UPSERT_REQUEST);

    expect(parsed).toEqual(VALID_CONFIG_UPSERT_REQUEST);
  });

  it("rejects empty allowedChains in POST /config payloads", () => {
    expect(() =>
      CrePolicyConfigUpsertRequestSchema.parse({
        ...VALID_CONFIG_UPSERT_REQUEST,
        allowedChains: [],
      }),
    ).toThrow();
  });

  it("rejects duplicate chains in POST /config payloads", () => {
    expect(() =>
      CrePolicyConfigUpsertRequestSchema.parse({
        ...VALID_CONFIG_UPSERT_REQUEST,
        allowedChains: ["base-sepolia", "base-sepolia"],
      }),
    ).toThrow();
  });

  it("rejects activation thresholds lower than zero", () => {
    expect(() =>
      CrePolicyConfigUpsertRequestSchema.parse({
        ...VALID_CONFIG_UPSERT_REQUEST,
        activationThreshold: -0.01,
      }),
    ).toThrow();
  });

  it("rejects activation thresholds greater than one", () => {
    expect(() =>
      CrePolicyConfigUpsertRequestSchema.parse({
        ...VALID_CONFIG_UPSERT_REQUEST,
        activationThreshold: 1.01,
      }),
    ).toThrow();
  });

  it("accepts valid POST /scenario payloads", () => {
    const parsed = CrePolicyScenarioUpdateRequestSchema.parse(
      VALID_SCENARIO_UPDATE_REQUEST,
    );

    expect(parsed).toEqual(VALID_SCENARIO_UPDATE_REQUEST);
  });

  it("rejects invalid scenario values", () => {
    expect(() =>
      CrePolicyScenarioUpdateRequestSchema.parse({
        ...VALID_SCENARIO_UPDATE_REQUEST,
        scenario: "degraded",
      }),
    ).toThrow();
  });

  it("accepts GET /config/:customerId params shape", () => {
    expect(
      CrePolicyGetConfigParamsSchema.parse({ customerId: VALID_CONFIG_SNAPSHOT.customerId }),
    ).toEqual({ customerId: VALID_CONFIG_SNAPSHOT.customerId });
  });

  it("accepts config snapshot response shape", () => {
    const parsed = CrePolicyConfigSnapshotSchema.parse(VALID_CONFIG_SNAPSHOT);

    expect(parsed).toEqual(VALID_CONFIG_SNAPSHOT);
  });

  it("accepts activation decisions with active and inactive chains", () => {
    expect(
      ActivationDecisionSchema.parse({
        thresholdUsed: 0.7,
        activeChains: ["base-sepolia", "arbitrum-sepolia"],
        inactiveChains: ["optimism-sepolia"],
      }),
    ).toEqual({
      thresholdUsed: 0.7,
      activeChains: ["base-sepolia", "arbitrum-sepolia"],
      inactiveChains: ["optimism-sepolia"],
    });
  });

  it("publishes full CRE Policy error code contract", () => {
    expect(CRE_POLICY_ERROR_CODES).toEqual([
      "UNAUTHORIZED",
      "INVALID_PAYLOAD",
      "UNSUPPORTED_CHAIN",
      "INVALID_SCENARIO",
      "CUSTOMER_NOT_FOUND",
      "STORE_UNAVAILABLE",
    ]);
  });

  it("maps invalid payload rejection for /config to contract-compatible error", () => {
    const result = parseCrePolicyConfigUpsertRequest({
      ...VALID_CONFIG_UPSERT_REQUEST,
      allowedChains: [],
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toEqual({
        errorCode: "INVALID_PAYLOAD",
        errorMessage: "invalid POST /config payload",
        requestId: VALID_CONFIG_UPSERT_REQUEST.requestId,
      });
      expect(CrePolicyErrorResponseSchema.parse(result.error)).toEqual(result.error);
    }
  });

  it("maps invalid payload rejection for /scenario to contract-compatible error", () => {
    const result = parseCrePolicyScenarioUpdateRequest({
      ...VALID_SCENARIO_UPDATE_REQUEST,
      scenario: "degraded",
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(CrePolicyErrorCodeSchema.parse(result.error.errorCode)).toBe("INVALID_PAYLOAD");
    }
  });
});
