import { describe, expect, it } from "vitest";

import {
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
  createPostConfigEndpoint,
  InMemorySupportedChainRegistry,
} from "../src";
import { createInMemoryConfigStorePort } from "./fixtures/in-memory-config-store-port";
import { POST_CONFIG_ENDPOINT_CASES } from "./fixtures/post-config-endpoint-cases";

describe("POST /config endpoint", () => {
  it("creates or updates snapshot and returns configVersion with updatedAt", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(
        POST_CONFIG_ENDPOINT_CASES.supportedChains,
      ),
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_CONFIG_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T22:10:00.000Z",
    });

    expect(response).toEqual({
      status: 200,
      body: {
        requestId: "req-29-valid-001",
        customerId: "customer-29",
        configVersion: 1,
        updatedAt: "2026-02-21T22:10:00.000Z",
      },
    });

    const auditRecords = await store.getAuditRecords({
      customerId: "customer-29",
      requestId: "req-29-valid-001",
    });

    expect(auditRecords).toEqual([
      {
        requestId: "req-29-valid-001",
        customerId: "customer-29",
        action: "config.upsert",
        timestamp: "2026-02-21T22:10:00.000Z",
        status: "accepted",
        configVersion: 1,
        errorCode: null,
        errorMessage: null,
      },
    ]);
  });

  it("is idempotent for same (customerId, requestId)", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(
        POST_CONFIG_ENDPOINT_CASES.supportedChains,
      ),
      authorize: async () => true,
    });

    const first = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_CONFIG_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T22:11:00.000Z",
    });

    const replay = await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        ...POST_CONFIG_ENDPOINT_CASES.validRequest,
        allowedChains: ["optimism-sepolia"],
      },
      now: "2026-02-21T22:12:00.000Z",
    });

    expect(first.status).toBe(200);
    expect(replay).toEqual(first);
  });

  it("rejects unsupported chains with UNSUPPORTED_CHAIN", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(
        POST_CONFIG_ENDPOINT_CASES.supportedChains,
      ),
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_CONFIG_ENDPOINT_CASES.unsupportedChainRequest,
      now: "2026-02-21T22:13:00.000Z",
    });

    expect(response).toEqual({
      status: 400,
      body: {
        errorCode: "UNSUPPORTED_CHAIN",
        errorMessage: "unsupported chain 'polygon-amoy'",
        requestId: "req-29-unsupported-001",
      },
    });

    const auditRecords = await store.getAuditRecords({
      customerId: "customer-29",
      requestId: "req-29-unsupported-001",
    });

    expect(auditRecords).toEqual([
      {
        requestId: "req-29-unsupported-001",
        customerId: "customer-29",
        action: "config.upsert",
        timestamp: "2026-02-21T22:13:00.000Z",
        status: "rejected",
        configVersion: null,
        errorCode: "UNSUPPORTED_CHAIN",
        errorMessage: "unsupported chain 'polygon-amoy'",
      },
    ]);
  });

  it("rejects invalid payloads with INVALID_PAYLOAD", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(
        POST_CONFIG_ENDPOINT_CASES.supportedChains,
      ),
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: {
        requestId: "req-29-invalid-001",
        customerId: "customer-29",
        allowedChains: [],
        active: true,
        activationThreshold: 0.7,
      },
      now: "2026-02-21T22:14:00.000Z",
    });

    expect(response).toEqual({
      status: 400,
      body: {
        errorCode: "INVALID_PAYLOAD",
        errorMessage: "invalid POST /config payload",
        requestId: "req-29-invalid-001",
      },
    });
  });

  it("rejects unauthorized requests with UNAUTHORIZED", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(
        POST_CONFIG_ENDPOINT_CASES.supportedChains,
      ),
      authorize: async () => false,
    });

    const response = await endpoint({
      authorizationHeader: undefined,
      payload: POST_CONFIG_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T22:15:00.000Z",
    });

    expect(response).toEqual({
      status: 401,
      body: {
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId: "req-29-valid-001",
      },
    });
  });

  it("maps store outages to STORE_UNAVAILABLE", async () => {
    const store = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase({
        failOnOperation: "upsertConfig",
      }),
    });
    const endpoint = createPostConfigEndpoint({
      store,
      registry: new InMemorySupportedChainRegistry(
        POST_CONFIG_ENDPOINT_CASES.supportedChains,
      ),
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      payload: POST_CONFIG_ENDPOINT_CASES.validRequest,
      now: "2026-02-21T22:16:00.000Z",
    });

    expect(response).toEqual({
      status: 503,
      body: {
        errorCode: "STORE_UNAVAILABLE",
        errorMessage: "config store unavailable",
        requestId: "req-29-valid-001",
      },
    });
  });
});
