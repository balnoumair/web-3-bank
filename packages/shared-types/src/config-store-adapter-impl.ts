import type {
  ConfigStorePort,
  ConfigStorePortAuditRecord,
  ConfigStorePortSnapshot,
  ConfigStorePortUpsertConfigInput,
  ConfigStorePortUpdateScenarioInput,
} from "./config-store-port";

type ConfigStoreAdapterOperation =
  | "upsertConfig"
  | "updateScenario"
  | "getConfigSnapshot"
  | "appendAuditRecord"
  | "getAuditRecords";

type ConfigStoreIdempotencyRecord = {
  customerId: string;
  requestId: string;
  operation: "upsertConfig" | "updateScenario";
  snapshot: ConfigStorePortSnapshot | null;
  effectiveAt: string | null;
};

export interface ConfigStoreDatabase {
  getSnapshot(customerId: string): Promise<ConfigStorePortSnapshot | null>;
  saveSnapshot(snapshot: ConfigStorePortSnapshot): Promise<void>;
  getIdempotencyRecord(
    customerId: string,
    requestId: string,
  ): Promise<ConfigStoreIdempotencyRecord | null>;
  saveIdempotencyRecord(record: ConfigStoreIdempotencyRecord): Promise<void>;
  appendAuditRecord(record: ConfigStorePortAuditRecord): Promise<void>;
  listAuditRecords(query: {
    customerId: string;
    requestId?: string;
  }): Promise<ConfigStorePortAuditRecord[]>;
}

export class ConfigStorePortStorageError extends Error {
  readonly errorCode = "STORE_UNAVAILABLE" as const;

  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "ConfigStorePortStorageError";
  }
}

function createSnapshotFromUpsert(args: {
  input: ConfigStorePortUpsertConfigInput;
  currentSnapshot: ConfigStorePortSnapshot | null;
}): ConfigStorePortSnapshot {
  return {
    customerId: args.input.customerId,
    configVersion: args.currentSnapshot ? args.currentSnapshot.configVersion + 1 : 1,
    updatedAt: args.input.timestamp,
    allowedChains: [...args.input.allowedChains],
    active: args.input.active,
    scenario: args.currentSnapshot?.scenario ?? "normal",
    activationThreshold: args.input.activationThreshold,
  };
}

function createSnapshotFromScenarioUpdate(args: {
  input: ConfigStorePortUpdateScenarioInput;
  currentSnapshot: ConfigStorePortSnapshot;
}): ConfigStorePortSnapshot {
  return {
    ...args.currentSnapshot,
    scenario: args.input.scenario,
    configVersion: args.currentSnapshot.configVersion + 1,
    updatedAt: args.input.timestamp,
  };
}

function toStorageError(error: unknown): ConfigStorePortStorageError {
  if (error instanceof ConfigStorePortStorageError) {
    return error;
  }

  if (error instanceof Error) {
    return new ConfigStorePortStorageError("config store operation failed", {
      cause: error.message,
    });
  }

  return new ConfigStorePortStorageError("config store operation failed", {
    cause: error,
  });
}

export function createConfigStoreAdapter(args: {
  db: ConfigStoreDatabase;
}): ConfigStorePort {
  async function withStorageError<T>(operation: () => Promise<T>): Promise<T> {
    try {
      return await operation();
    } catch (error) {
      throw toStorageError(error);
    }
  }

  return {
    async upsertConfig(input) {
      return withStorageError(async () => {
        const previousIdempotency = await args.db.getIdempotencyRecord(
          input.customerId,
          input.requestId,
        );

        if (previousIdempotency && previousIdempotency.operation === "upsertConfig") {
          if (!previousIdempotency.snapshot) {
            throw new Error("idempotency record missing snapshot for upsertConfig");
          }

          return {
            snapshot: previousIdempotency.snapshot,
            idempotentReplay: true,
          };
        }

        const currentSnapshot = await args.db.getSnapshot(input.customerId);
        const nextSnapshot = createSnapshotFromUpsert({
          input,
          currentSnapshot,
        });

        await args.db.saveSnapshot(nextSnapshot);
        await args.db.saveIdempotencyRecord({
          customerId: input.customerId,
          requestId: input.requestId,
          operation: "upsertConfig",
          snapshot: nextSnapshot,
          effectiveAt: null,
        });

        return {
          snapshot: nextSnapshot,
          idempotentReplay: false,
        };
      });
    },

    async updateScenario(input) {
      return withStorageError(async () => {
        const previousIdempotency = await args.db.getIdempotencyRecord(
          input.customerId,
          input.requestId,
        );

        if (previousIdempotency && previousIdempotency.operation === "updateScenario") {
          return {
            snapshot: previousIdempotency.snapshot,
            effectiveAt: previousIdempotency.effectiveAt,
            appliesToRun: "next" as const,
            idempotentReplay: true,
          };
        }

        const currentSnapshot = await args.db.getSnapshot(input.customerId);
        const nextSnapshot = currentSnapshot
          ? createSnapshotFromScenarioUpdate({
            input,
            currentSnapshot,
          })
          : null;

        if (nextSnapshot) {
          await args.db.saveSnapshot(nextSnapshot);
        }

        const effectiveAt = nextSnapshot ? input.timestamp : null;
        await args.db.saveIdempotencyRecord({
          customerId: input.customerId,
          requestId: input.requestId,
          operation: "updateScenario",
          snapshot: nextSnapshot,
          effectiveAt,
        });

        return {
          snapshot: nextSnapshot,
          effectiveAt,
          appliesToRun: "next" as const,
          idempotentReplay: false,
        };
      });
    },

    async getConfigSnapshot(customerId) {
      return withStorageError(async () => args.db.getSnapshot(customerId));
    },

    async appendAuditRecord(record) {
      await withStorageError(async () => args.db.appendAuditRecord(record));
    },

    async getAuditRecords(query) {
      return withStorageError(async () => args.db.listAuditRecords(query));
    },
  };
}

export function createInMemoryConfigStoreDatabase(args?: {
  failOnOperation?: ConfigStoreAdapterOperation;
}): ConfigStoreDatabase {
  const snapshotsByCustomer = new Map<string, ConfigStorePortSnapshot>();
  const idempotencyByKey = new Map<string, ConfigStoreIdempotencyRecord>();
  const auditRecords: ConfigStorePortAuditRecord[] = [];

  const shouldFail = (operation: ConfigStoreAdapterOperation): void => {
    if (args?.failOnOperation === operation) {
      throw new Error("simulated supabase outage");
    }
  };

  const toIdempotencyKey = (customerId: string, requestId: string): string =>
    `${customerId}::${requestId}`;

  return {
    async getSnapshot(customerId) {
      shouldFail("getConfigSnapshot");
      return snapshotsByCustomer.get(customerId) ?? null;
    },

    async saveSnapshot(snapshot) {
      shouldFail("upsertConfig");
      snapshotsByCustomer.set(snapshot.customerId, snapshot);
    },

    async getIdempotencyRecord(customerId, requestId) {
      return idempotencyByKey.get(toIdempotencyKey(customerId, requestId)) ?? null;
    },

    async saveIdempotencyRecord(record) {
      if (args?.failOnOperation === record.operation) {
        throw new Error("simulated supabase outage");
      }

      idempotencyByKey.set(
        toIdempotencyKey(record.customerId, record.requestId),
        record,
      );
    },

    async appendAuditRecord(record) {
      shouldFail("appendAuditRecord");
      auditRecords.push(record);
    },

    async listAuditRecords(query) {
      shouldFail("getAuditRecords");
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
