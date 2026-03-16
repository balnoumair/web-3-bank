import type {
  ConfigStorePort,
  ConfigStorePortAuditRecord,
  ConfigStorePortSnapshot,
} from "@repo/shared-types";
import { InMemorySupportedChainRegistry } from "@repo/shared-types";
import { describe, expect, it } from "vitest";

import { createCrePolicyToEvaluatorNextRunValidationRunner } from "../src";

function createInMemoryStore(): ConfigStorePort {
  const snapshots = new Map<string, ConfigStorePortSnapshot>();
  const auditRecords: ConfigStorePortAuditRecord[] = [];

  return {
    async upsertConfig(input) {
      const current = snapshots.get(input.customerId);
      const nextVersion = current ? current.configVersion + 1 : 1;

      const nextSnapshot: ConfigStorePortSnapshot = {
        customerId: input.customerId,
        configVersion: nextVersion,
        updatedAt: input.timestamp,
        allowedChains: [...input.allowedChains],
        active: input.active,
        scenario: current?.scenario ?? "normal",
      };

      snapshots.set(input.customerId, nextSnapshot);
      return {
        snapshot: nextSnapshot,
        idempotentReplay: false,
      };
    },

    async updateScenario(input) {
      const current = snapshots.get(input.customerId);
      if (!current) {
        return {
          snapshot: null,
          effectiveAt: null,
          appliesToRun: "next" as const,
          idempotentReplay: false,
        };
      }

      const nextSnapshot: ConfigStorePortSnapshot = {
        ...current,
        scenario: input.scenario,
        configVersion: current.configVersion + 1,
        updatedAt: input.timestamp,
      };

      snapshots.set(input.customerId, nextSnapshot);
      return {
        snapshot: nextSnapshot,
        effectiveAt: input.timestamp,
        appliesToRun: "next" as const,
        idempotentReplay: false,
      };
    },

    async getConfigSnapshot(customerId) {
      return snapshots.get(customerId) ?? null;
    },

    async appendAuditRecord(record) {
      auditRecords.push(record);
    },

    async getAuditRecords(query) {
      return auditRecords.filter((record) => {
        if (record.customerId !== query.customerId) {
          return false;
        }

        if (query.requestId && record.requestId !== query.requestId) {
          return false;
        }

        return true;
      });
    },
  };
}

describe("CRE Policy -> CRE Evaluator next-run validation", () => {
  it("updates snapshot and audit trail, then consumes latest committed snapshot", async () => {
    const runner = createCrePolicyToEvaluatorNextRunValidationRunner({
      store: createInMemoryStore(),
      registry: new InMemorySupportedChainRegistry(["base-sepolia", "arbitrum-sepolia"]),
      authorize: () => true,
      runIdPrefix: "dev-26",
      requestIdPrefix: "req-26",
      baseTimestamp: "2026-02-22T12:00:00.000Z",
    });

    const evidence = await runner.runValidation({
      customerId: "customer-26",
      allowedChains: ["base-sepolia", "arbitrum-sepolia"],
      active: true,
      scenario: "congested",
    });

    expect(evidence.runId).toBe("dev-26-001");
    expect(evidence.configVersion).toBe(2);
    expect(evidence.snapshot.scenario).toBe("congested");
    expect(evidence.snapshot.configVersion).toBe(2);
    expect(evidence.correlation).toEqual({
      requestId: "req-26-scenario-001",
      configVersion: 2,
      runId: "dev-26-001",
    });
    expect(evidence.auditRecord).toMatchObject({
      requestId: "req-26-scenario-001",
      action: "scenario.update",
      status: "accepted",
      configVersion: 2,
    });
    expect(evidence.auditTrail.map((record) => record.requestId)).toEqual([
      "req-26-config-001",
      "req-26-scenario-001",
    ]);
  });

  it("is repeatable without manual data repair", async () => {
    const runner = createCrePolicyToEvaluatorNextRunValidationRunner({
      store: createInMemoryStore(),
      registry: new InMemorySupportedChainRegistry(["base-sepolia", "arbitrum-sepolia"]),
      authorize: () => true,
      runIdPrefix: "dev-26",
      requestIdPrefix: "req-26",
      baseTimestamp: "2026-02-22T12:00:00.000Z",
    });

    const first = await runner.runValidation({
      customerId: "customer-26",
      allowedChains: ["base-sepolia", "arbitrum-sepolia"],
      active: true,
      scenario: "congested",
    });

    const second = await runner.runValidation({
      customerId: "customer-26",
      allowedChains: ["base-sepolia"],
      active: true,
      scenario: "normal",
    });

    expect(first.runId).toBe("dev-26-001");
    expect(second.runId).toBe("dev-26-002");
    expect(first.requestId).toBe("req-26-scenario-001");
    expect(second.requestId).toBe("req-26-scenario-002");
    expect(first.configVersion).toBe(2);
    expect(second.configVersion).toBe(4);
    expect(first.correlation.runId).toBe(first.runId);
    expect(second.correlation.runId).toBe(second.runId);
  });
});
