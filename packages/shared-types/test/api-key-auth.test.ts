import { describe, expect, it } from "vitest";

import {
  createApiKeyAuthorizer,
  createPostConfigEndpoint,
  InMemoryApiKeyStore,
  InMemorySupportedChainRegistry,
} from "../src";
import { createInMemoryConfigStorePort } from "./fixtures/in-memory-config-store-port";

describe("api key auth middleware", () => {
  it("accepts only active bearer tokens", async () => {
    const keyStore = new InMemoryApiKeyStore([
      {
        keyId: "key-42-active",
        token: "cre-live-42",
        status: "active",
        createdAt: "2026-02-22T12:00:00.000Z",
        revokedAt: null,
      },
      {
        keyId: "key-42-revoked",
        token: "cre-revoked-42",
        status: "revoked",
        createdAt: "2026-02-22T11:00:00.000Z",
        revokedAt: "2026-02-22T11:30:00.000Z",
      },
    ]);

    const authorize = createApiKeyAuthorizer({ store: keyStore });

    await expect(authorize(undefined)).resolves.toBe(false);
    await expect(authorize("Token cre-live-42")).resolves.toBe(false);
    await expect(authorize("Bearer cre-missing-42")).resolves.toBe(false);
    await expect(authorize("Bearer cre-revoked-42")).resolves.toBe(false);
    await expect(authorize("Bearer cre-live-42")).resolves.toBe(true);
  });

  it("supports key rotation without changing write contracts or idempotency", async () => {
    const store = createInMemoryConfigStorePort();
    const registry = new InMemorySupportedChainRegistry(["base-sepolia"]);
    const keyStore = new InMemoryApiKeyStore([
      {
        keyId: "key-42-old",
        token: "cre-old-42",
        status: "active",
        createdAt: "2026-02-22T12:10:00.000Z",
        revokedAt: null,
      },
    ]);
    const authorize = createApiKeyAuthorizer({ store: keyStore });
    const endpoint = createPostConfigEndpoint({
      store,
      registry,
      authorize,
    });

    const first = await endpoint({
      authorizationHeader: "Bearer cre-old-42",
      now: "2026-02-22T12:11:00.000Z",
      payload: {
        requestId: "req-42-rotate-001",
        customerId: "customer-42",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.7,
      },
    });

    keyStore.rotateKey({
      previousKeyId: "key-42-old",
      nextKeyId: "key-42-new",
      nextToken: "cre-new-42",
      rotatedAt: "2026-02-22T12:12:00.000Z",
    });

    const replayWithNewKey = await endpoint({
      authorizationHeader: "Bearer cre-new-42",
      now: "2026-02-22T12:12:01.000Z",
      payload: {
        requestId: "req-42-rotate-001",
        customerId: "customer-42",
        allowedChains: ["base-sepolia"],
        active: false,
        activationThreshold: 0.4,
      },
    });

    const oldKeyRejected = await endpoint({
      authorizationHeader: "Bearer cre-old-42",
      now: "2026-02-22T12:12:02.000Z",
      payload: {
        requestId: "req-42-rotate-002",
        customerId: "customer-42",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.7,
      },
    });

    const secondWithNewKey = await endpoint({
      authorizationHeader: "Bearer cre-new-42",
      now: "2026-02-22T12:12:03.000Z",
      payload: {
        requestId: "req-42-rotate-003",
        customerId: "customer-42",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.7,
      },
    });

    expect(first.status).toBe(200);
    expect(replayWithNewKey).toEqual(first);
    expect(oldKeyRejected).toEqual({
      status: 401,
      body: {
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId: "req-42-rotate-002",
      },
    });
    expect(secondWithNewKey.status).toBe(200);

    if (first.status === 200 && secondWithNewKey.status === 200) {
      expect(first.body.configVersion).toBe(1);
      expect(secondWithNewKey.body.configVersion).toBe(2);
    }
  });
});
