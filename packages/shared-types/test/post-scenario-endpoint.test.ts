import { describe, expect, it } from "vitest";

import {
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
  createPostScenarioEndpoint,
} from "../src";
import { createInMemoryConfigStorePort } from "./fixtures/in-memory-config-store-port";
import { POST_SCENARIO_ENDPOINT_CASES } from "./fixtures/post-scenario-endpoint-cases";

describe("POST /scenario endpoint", () => {
  it("accepts valid scenario and returns effectiveAt with appliesToRun next", async () => {
    const store = createInMemoryConfigStorePort();
    await store.upsertConfig({
      requestId: "req-30-bootstrap",
      customerId: "customer-30",
      allowedChains: ["base-sepolia"],
      active: true,
      activationThreshold: 0.7,
      timestamp: "2026-02-21T23:00:00.000Z",
    });

    const endpoint = createPostScenarioEndpoint({
      store,
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_SCENARIO_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T23:01:00.000Z",
    });

    expect(response).toEqual({
      status: 200,
      body: {
        requestId: "req-30-valid-001",
        customerId: "customer-30",
        scenario: "congested",
        configVersion: 2,
        effectiveAt: "2026-02-21T23:01:00.000Z",
        appliesToRun: "next",
      },
    });

    const auditRecords = await store.getAuditRecords({
      customerId: "customer-30",
      requestId: "req-30-valid-001",
    });

    expect(auditRecords).toEqual([
      {
        requestId: "req-30-valid-001",
        customerId: "customer-30",
        action: "scenario.update",
        timestamp: "2026-02-21T23:01:00.000Z",
        status: "accepted",
        configVersion: 2,
        errorCode: null,
        errorMessage: null,
      },
    ]);
  });

  it("is idempotent for same (customerId, requestId)", async () => {
    const store = createInMemoryConfigStorePort();
    await store.upsertConfig({
      requestId: "req-30-bootstrap-idem",
      customerId: "customer-30",
      allowedChains: ["base-sepolia"],
      active: true,
      activationThreshold: 0.7,
      timestamp: "2026-02-21T23:05:00.000Z",
    });

    const endpoint = createPostScenarioEndpoint({
      store,
      authorize: async () => true,
    });

    const first = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_SCENARIO_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T23:06:00.000Z",
    });

    const replay = await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        ...POST_SCENARIO_ENDPOINT_CASES.validRequest,
        scenario: "normal",
      },
      now: "2026-02-21T23:07:00.000Z",
    });

    expect(first.status).toBe(200);
    expect(replay).toEqual(first);
  });

  it("rejects unsupported scenario values as INVALID_SCENARIO", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostScenarioEndpoint({
      store,
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_SCENARIO_ENDPOINT_CASES.invalidScenarioRequest,
      now: "2026-02-21T23:08:00.000Z",
    });

    expect(response).toEqual({
      status: 400,
      body: {
        errorCode: "INVALID_SCENARIO",
        errorMessage: "invalid scenario value",
        requestId: "req-30-invalid-scenario",
      },
    });

    const auditRecords = await store.getAuditRecords({
      customerId: "customer-30",
      requestId: "req-30-invalid-scenario",
    });

    expect(auditRecords).toEqual([
      {
        requestId: "req-30-invalid-scenario",
        customerId: "customer-30",
        action: "scenario.update",
        timestamp: "2026-02-21T23:08:00.000Z",
        status: "rejected",
        configVersion: null,
        errorCode: "INVALID_SCENARIO",
        errorMessage: "invalid scenario value",
      },
    ]);
  });

  it("returns CUSTOMER_NOT_FOUND for unknown customers", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostScenarioEndpoint({
      store,
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_SCENARIO_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T23:09:00.000Z",
    });

    expect(response).toEqual({
      status: 404,
      body: {
        errorCode: "CUSTOMER_NOT_FOUND",
        errorMessage: "customer not found",
        requestId: "req-30-valid-001",
      },
    });
  });

  it("rejects unauthorized requests with UNAUTHORIZED", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostScenarioEndpoint({
      store,
      authorize: async () => false,
    });

    const response = await endpoint({
      authorizationHeader: undefined,
      payload: POST_SCENARIO_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T23:10:00.000Z",
    });

    expect(response).toEqual({
      status: 401,
      body: {
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId: "req-30-valid-001",
      },
    });
  });

  it("maps store outages to STORE_UNAVAILABLE", async () => {
    const store = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase({
        failOnOperation: "updateScenario",
      }),
    });

    await expect(
      store.upsertConfig({
        requestId: "req-30-bootstrap-failure",
        customerId: "customer-30",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.7,
        timestamp: "2026-02-21T23:11:00.000Z",
      }),
    ).resolves.toBeDefined();

    const endpoint = createPostScenarioEndpoint({
      store,
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_SCENARIO_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T23:12:00.000Z",
    });

    expect(response).toEqual({
      status: 503,
      body: {
        errorCode: "STORE_UNAVAILABLE",
        errorMessage: "config store unavailable",
        requestId: "req-30-valid-001",
      },
    });
  });
});
