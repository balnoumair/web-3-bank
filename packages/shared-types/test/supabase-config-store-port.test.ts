import { describe, expect, it } from "vitest";

import {
  ConfigStorePortStorageError,
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
} from "../src";

describe("config store adapter", () => {
  it("maps adapter storage failures to STORE_UNAVAILABLE", async () => {
    const db = createInMemoryConfigStoreDatabase({
      failOnOperation: "upsertConfig",
    });
    const store = createConfigStoreAdapter({ db });

    await expect(
      store.upsertConfig({
        requestId: "req-27-store-down",
        customerId: "customer-27",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.7,
        timestamp: "2026-02-21T22:00:00.000Z",
      }),
    ).rejects.toBeInstanceOf(ConfigStorePortStorageError);

    await expect(
      store.upsertConfig({
        requestId: "req-27-store-down",
        customerId: "customer-27",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.7,
        timestamp: "2026-02-21T22:00:00.000Z",
      }),
    ).rejects.toMatchObject({
      errorCode: "STORE_UNAVAILABLE",
      message: "config store operation failed",
      cause: "simulated supabase outage",
    });
  });
});
