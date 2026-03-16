import type {
  ConfigStorePort,
  ConfigStorePortAuditRecord,
  ConfigStorePortSnapshot,
  ConfigStorePortUpsertConfigInput,
  ConfigStorePortUpdateScenarioInput,
} from "../../src";

type IdempotencyEntry = {
  operation: "upsertConfig" | "updateScenario";
  result: {
    snapshot: ConfigStorePortSnapshot | null;
    effectiveAt: string | null;
  };
};

export function createInMemoryConfigStorePort(): ConfigStorePort {
  const snapshotsByCustomer = new Map<string, ConfigStorePortSnapshot>();
  const auditRecords: ConfigStorePortAuditRecord[] = [];
  const idempotency = new Map<string, IdempotencyEntry>();

  const toIdempotencyKey = (customerId: string, requestId: string): string =>
    `${customerId}::${requestId}`;

  const applyUpsertConfig = (
    input: ConfigStorePortUpsertConfigInput,
  ): ConfigStorePortSnapshot => {
    const current = snapshotsByCustomer.get(input.customerId);
    const nextVersion = current ? current.configVersion + 1 : 1;

    const nextSnapshot: ConfigStorePortSnapshot = {
      customerId: input.customerId,
      configVersion: nextVersion,
      updatedAt: input.timestamp,
      allowedChains: [...input.allowedChains],
      active: input.active,
      scenario: current?.scenario ?? "normal",
      activationThreshold: input.activationThreshold,
    };

    snapshotsByCustomer.set(input.customerId, nextSnapshot);
    return nextSnapshot;
  };

  const applyUpdateScenario = (
    input: ConfigStorePortUpdateScenarioInput,
  ): ConfigStorePortSnapshot | null => {
    const current = snapshotsByCustomer.get(input.customerId);
    if (!current) {
      return null;
    }

    const nextSnapshot: ConfigStorePortSnapshot = {
      ...current,
      scenario: input.scenario,
      configVersion: current.configVersion + 1,
      updatedAt: input.timestamp,
    };

    snapshotsByCustomer.set(input.customerId, nextSnapshot);
    return nextSnapshot;
  };

  return {
    async upsertConfig(input) {
      const key = toIdempotencyKey(input.customerId, input.requestId);
      const previous = idempotency.get(key);
      if (previous && previous.operation === "upsertConfig") {
        if (!previous.result.snapshot) {
          throw new Error("invalid idempotency state for upsertConfig");
        }

        return {
          snapshot: previous.result.snapshot,
          idempotentReplay: true,
        };
      }

      const snapshot = applyUpsertConfig(input);
      idempotency.set(key, {
        operation: "upsertConfig",
        result: {
          snapshot,
          effectiveAt: null,
        },
      });

      return {
        snapshot,
        idempotentReplay: false,
      };
    },

    async updateScenario(input) {
      const key = toIdempotencyKey(input.customerId, input.requestId);
      const previous = idempotency.get(key);
      if (previous && previous.operation === "updateScenario") {
        return {
          snapshot: previous.result.snapshot,
          effectiveAt: previous.result.effectiveAt,
          appliesToRun: "next",
          idempotentReplay: true,
        };
      }

      const snapshot = applyUpdateScenario(input);
      const effectiveAt = snapshot ? input.timestamp : null;
      idempotency.set(key, {
        operation: "updateScenario",
        result: {
          snapshot,
          effectiveAt,
        },
      });

      return {
        snapshot,
        effectiveAt,
        appliesToRun: "next",
        idempotentReplay: false,
      };
    },

    async getConfigSnapshot(customerId) {
      return snapshotsByCustomer.get(customerId) ?? null;
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
