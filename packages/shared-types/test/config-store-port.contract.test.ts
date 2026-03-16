import { describe, expect, it } from "vitest";

import {
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
  type ConfigStorePort,
} from "../src";
import { createInMemoryConfigStorePort } from "./fixtures/in-memory-config-store-port";

function runConfigStorePortContractTests(
  name: string,
  createAdapter: () => ConfigStorePort,
): void {
  describe(name, () => {
    it("upserts config and returns versioned snapshot", async () => {
      const store = createAdapter();

      const result = await store.upsertConfig({
        requestId: "req-34-upsert-001",
        customerId: "customer-34",
        allowedChains: ["base-sepolia", "arbitrum-sepolia"],
        active: true,
        activationThreshold: 0.7,
        timestamp: "2026-02-21T21:00:00.000Z",
      });

      expect(result.idempotentReplay).toBe(false);
      expect(result.snapshot.customerId).toBe("customer-34");
      expect(result.snapshot.configVersion).toBe(1);
      expect(result.snapshot.allowedChains).toEqual([
        "base-sepolia",
        "arbitrum-sepolia",
      ]);
      expect(result.snapshot.scenario).toBe("normal");
      expect(result.snapshot.activationThreshold).toBe(0.7);
    });

    it("enforces idempotency on (customerId, requestId) for config upsert", async () => {
      const store = createAdapter();

      const first = await store.upsertConfig({
        requestId: "req-34-upsert-idem",
        customerId: "customer-34",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.6,
        timestamp: "2026-02-21T21:05:00.000Z",
      });

      const replay = await store.upsertConfig({
        requestId: "req-34-upsert-idem",
        customerId: "customer-34",
        allowedChains: ["optimism-sepolia"],
        active: false,
        activationThreshold: 0.2,
        timestamp: "2026-02-21T21:05:30.000Z",
      });

      expect(first.idempotentReplay).toBe(false);
      expect(replay.idempotentReplay).toBe(true);
      expect(replay.snapshot).toEqual(first.snapshot);
    });

    it("updates scenario and applies idempotency on (customerId, requestId)", async () => {
      const store = createAdapter();

      await store.upsertConfig({
        requestId: "req-34-bootstrap",
        customerId: "customer-34",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.75,
        timestamp: "2026-02-21T21:10:00.000Z",
      });

      const first = await store.updateScenario({
        requestId: "req-34-scenario-idem",
        customerId: "customer-34",
        scenario: "congested",
        timestamp: "2026-02-21T21:11:00.000Z",
      });

      const replay = await store.updateScenario({
        requestId: "req-34-scenario-idem",
        customerId: "customer-34",
        scenario: "normal",
        timestamp: "2026-02-21T21:12:00.000Z",
      });

      expect(first.snapshot).not.toBeNull();
      expect(first.idempotentReplay).toBe(false);
      expect(first.snapshot?.scenario).toBe("congested");
      expect(first.snapshot?.activationThreshold).toBe(0.75);
      expect(replay.idempotentReplay).toBe(true);
      expect(replay.snapshot).toEqual(first.snapshot);
    });

    it("reads latest config snapshot and appends audit records", async () => {
      const store = createAdapter();

      await store.upsertConfig({
        requestId: "req-34-snapshot-001",
        customerId: "customer-34",
        allowedChains: ["base-sepolia"],
        active: true,
        activationThreshold: 0.55,
        timestamp: "2026-02-21T21:20:00.000Z",
      });

      await store.appendAuditRecord({
        requestId: "req-34-audit-001",
        customerId: "customer-34",
        action: "config.upsert",
        timestamp: "2026-02-21T21:20:01.000Z",
        status: "accepted",
        configVersion: 1,
        errorCode: null,
        errorMessage: null,
      });

      const snapshot = await store.getConfigSnapshot("customer-34");
      const auditRecords = await store.getAuditRecords({
        customerId: "customer-34",
        requestId: "req-34-audit-001",
      });

      expect(snapshot?.configVersion).toBe(1);
      expect(snapshot?.customerId).toBe("customer-34");
      expect(snapshot?.activationThreshold).toBe(0.55);
      expect(auditRecords).toEqual([
        {
          requestId: "req-34-audit-001",
          customerId: "customer-34",
          action: "config.upsert",
          timestamp: "2026-02-21T21:20:01.000Z",
          status: "accepted",
          configVersion: 1,
          errorCode: null,
          errorMessage: null,
        },
      ]);
    });
  });
}

describe("ConfigStorePort contract", () => {
  runConfigStorePortContractTests("in-memory mock adapter", () =>
    createInMemoryConfigStorePort(),
  );

  runConfigStorePortContractTests("config store adapter", () =>
    createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase(),
    }),
  );
});
