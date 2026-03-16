import { describe, expect, it } from "vitest";

import {
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
  createGetConfigEndpoint,
  createPostConfigEndpoint,
  createPostScenarioEndpoint,
  InMemorySupportedChainRegistry,
} from "../src";
import { createInMemoryConfigStorePort } from "./fixtures/in-memory-config-store-port";

describe("CRE Policy failure modes", () => {
  it("maps auth, payload, chain, customer, and storage failures to contract statuses", async () => {
    const inMemoryStore = createInMemoryConfigStorePort();
    const postConfig = createPostConfigEndpoint({
      store: inMemoryStore,
      registry: new InMemorySupportedChainRegistry(["base-sepolia"]),
      authorize: async (header) => Boolean(header),
    });
    const postScenario = createPostScenarioEndpoint({
      store: inMemoryStore,
      authorize: async (header) => Boolean(header),
    });
    const getConfig = createGetConfigEndpoint({
      store: inMemoryStore,
      authorize: async (header) => Boolean(header),
    });

    const unauthorized = await postConfig({
      authorizationHeader: undefined,
      payload: {
        requestId: "req-36-auth",
        customerId: "customer-36",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.7,
      },
    });

    const invalidPayload = await postConfig({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-invalid-payload",
        customerId: "customer-36",
        allowedChains: [],
        active: true,
        activationThreshold: 0.7,
      },
    });

    const unsupportedChain = await postConfig({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-unsupported-chain",
        customerId: "customer-36",
        allowedChains: ["polygon-amoy"],
        active: true,
        activationThreshold: 0.7,
      },
    });

    const invalidScenario = await postScenario({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-invalid-scenario",
        customerId: "customer-36",
        scenario: "degraded",
      },
    });

    const customerNotFound = await getConfig({
      authorizationHeader: "Bearer token",
      params: { customerId: "customer-36-missing" },
    });

    const unavailableStore = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase({ failOnOperation: "getConfigSnapshot" }),
    });
    const unavailableGet = createGetConfigEndpoint({
      store: unavailableStore,
      authorize: async () => true,
    });
    const storeUnavailable = await unavailableGet({
      authorizationHeader: "Bearer token",
      params: { customerId: "customer-36" },
    });

    expect(unauthorized.status).toBe(401);
    expect(invalidPayload.status).toBe(400);
    expect(unsupportedChain.status).toBe(400);
    expect(invalidScenario.status).toBe(400);
    expect(customerNotFound.status).toBe(404);
    expect(storeUnavailable.status).toBe(503);

    expect(unauthorized.body.errorCode).toBe("UNAUTHORIZED");
    expect(invalidPayload.body.errorCode).toBe("INVALID_PAYLOAD");
    expect(unsupportedChain.body.errorCode).toBe("UNSUPPORTED_CHAIN");
    expect(invalidScenario.body.errorCode).toBe("INVALID_SCENARIO");
    expect(customerNotFound.body.errorCode).toBe("CUSTOMER_NOT_FOUND");
    expect(storeUnavailable.body.errorCode).toBe("STORE_UNAVAILABLE");
  });

  it("keeps idempotency scoped to (customerId, requestId)", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(["base-sepolia", "arbitrum-sepolia"]),
      authorize: async () => true,
    });

    const firstCustomerFirst = await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-same-id",
        customerId: "customer-36-a",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.65,
      },
      now: "2026-02-22T01:00:00.000Z",
    });

    const firstCustomerReplay = await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-same-id",
        customerId: "customer-36-a",
        allowedChains: ["arbitrum-sepolia"],
        active: false,
        activationThreshold: 0.2,
      },
      now: "2026-02-22T01:01:00.000Z",
    });

    const secondCustomerSameRequestId = await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-same-id",
        customerId: "customer-36-b",
        allowedChains: ["arbitrum-sepolia"],
        active: true,
        activationThreshold: 0.8,
      },
      now: "2026-02-22T01:02:00.000Z",
    });

    expect(firstCustomerFirst.status).toBe(200);
    expect(firstCustomerReplay).toEqual(firstCustomerFirst);
    expect(secondCustomerSameRequestId.status).toBe(200);

    if (firstCustomerFirst.status === 200 && secondCustomerSameRequestId.status === 200) {
      expect(firstCustomerFirst.body.customerId).toBe("customer-36-a");
      expect(secondCustomerSameRequestId.body.customerId).toBe("customer-36-b");
    }
  });

  it("writes rejected audit records for invalid and unsupported config writes", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(["base-sepolia"]),
      authorize: async () => true,
    });

    await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-rejected-invalid",
        customerId: "customer-36-audit",
        allowedChains: [],
        active: true,
        activationThreshold: 0.7,
      },
      now: "2026-02-22T01:10:00.000Z",
    });

    await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-36-rejected-unsupported",
        customerId: "customer-36-audit",
        allowedChains: ["polygon-amoy"],
        active: true,
        activationThreshold: 0.7,
      },
      now: "2026-02-22T01:11:00.000Z",
    });

    const invalidAudit = await store.getAuditRecords({
      customerId: "customer-36-audit",
      requestId: "req-36-rejected-invalid",
    });
    const unsupportedAudit = await store.getAuditRecords({
      customerId: "customer-36-audit",
      requestId: "req-36-rejected-unsupported",
    });

    expect(invalidAudit[0]?.status).toBe("rejected");
    expect(invalidAudit[0]?.errorCode).toBe("INVALID_PAYLOAD");
    expect(invalidAudit[0]?.configVersion).toBeNull();

    expect(unsupportedAudit[0]?.status).toBe("rejected");
    expect(unsupportedAudit[0]?.errorCode).toBe("UNSUPPORTED_CHAIN");
    expect(unsupportedAudit[0]?.configVersion).toBeNull();
  });
});
