import { describe, expect, it } from "vitest";

import { createGetConfigEndpoint } from "../src";
import { createInMemoryConfigStorePort } from "./fixtures/in-memory-config-store-port";

describe("GET /config/:customerId endpoint", () => {
  it("returns contract-compliant ConfigSnapshot for existing customer", async () => {
    const store = createInMemoryConfigStorePort();
    await store.upsertConfig({
      requestId: "req-31-bootstrap-001",
      customerId: "customer-31",
      allowedChains: ["base-sepolia", "arbitrum-sepolia"],
      active: true,
      timestamp: "2026-02-21T23:30:00.000Z",
    });

    const endpoint = createGetConfigEndpoint({
      store,
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      params: { customerId: "customer-31" },
    });

    expect(response.status).toBe(200);
    if (response.status === 200) {
      expect(response.body.customerId).toBe("customer-31");
      expect(response.body.configVersion).toBe(1);
      expect(response.body.updatedAt).toBe("2026-02-21T23:30:00.000Z");
      expect(response.body.allowedChains).toEqual([
        "base-sepolia",
        "arbitrum-sepolia",
      ]);
      expect(response.body.active).toBe(true);
      expect(response.body.scenario).toBe("normal");
    }
  });

  it("returns CUSTOMER_NOT_FOUND for missing customer", async () => {
    const store = createInMemoryConfigStorePort();
    const endpoint = createGetConfigEndpoint({
      store,
      authorize: async () => true,
    });

    const response = await endpoint({
      authorizationHeader: "Bearer token",
      params: { customerId: "customer-31-missing" },
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
