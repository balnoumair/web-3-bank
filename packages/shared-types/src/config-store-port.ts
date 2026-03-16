import * as z from "zod";

import { CrePolicyConfigSnapshotSchema } from "./intake/cre-policy-api-contract";
import { ScenarioNameSchema } from "./core-types";

export const ConfigStorePortAuditActionSchema = z.enum([
  "config.upsert",
  "scenario.update",
]);

export type ConfigStorePortAuditAction = z.infer<
  typeof ConfigStorePortAuditActionSchema
>;

export const ConfigStorePortAuditStatusSchema = z.enum(["accepted", "rejected"]);

export type ConfigStorePortAuditStatus = z.infer<
  typeof ConfigStorePortAuditStatusSchema
>;

export const ConfigStorePortSnapshotSchema = CrePolicyConfigSnapshotSchema;

export type ConfigStorePortSnapshot = z.infer<typeof ConfigStorePortSnapshotSchema>;

export const ConfigStorePortUpsertConfigInputSchema = z.object({
  requestId: z.string(),
  customerId: z.string(),
  allowedChains: z.array(z.string()).min(1),
  active: z.boolean(),
  activationThreshold: z.number().min(0).max(1),
  timestamp: z.string(),
});

export type ConfigStorePortUpsertConfigInput = z.infer<
  typeof ConfigStorePortUpsertConfigInputSchema
>;

export const ConfigStorePortUpdateScenarioInputSchema = z.object({
  requestId: z.string(),
  customerId: z.string(),
  scenario: ScenarioNameSchema,
  timestamp: z.string(),
});

export type ConfigStorePortUpdateScenarioInput = z.infer<
  typeof ConfigStorePortUpdateScenarioInputSchema
>;

export const ConfigStorePortAuditRecordSchema = z.object({
  requestId: z.string(),
  customerId: z.string(),
  action: ConfigStorePortAuditActionSchema,
  timestamp: z.string(),
  status: ConfigStorePortAuditStatusSchema,
  configVersion: z.number().int().nonnegative().nullable(),
  errorCode: z.string().nullable(),
  errorMessage: z.string().nullable(),
});

export type ConfigStorePortAuditRecord = z.infer<
  typeof ConfigStorePortAuditRecordSchema
>;

export interface ConfigStorePort {
  upsertConfig(input: ConfigStorePortUpsertConfigInput): Promise<{
    snapshot: ConfigStorePortSnapshot;
    idempotentReplay: boolean;
  }>;

  updateScenario(input: ConfigStorePortUpdateScenarioInput): Promise<{
    snapshot: ConfigStorePortSnapshot | null;
    effectiveAt: string | null;
    appliesToRun: "next";
    idempotentReplay: boolean;
  }>;

  getConfigSnapshot(customerId: string): Promise<ConfigStorePortSnapshot | null>;

  appendAuditRecord(record: ConfigStorePortAuditRecord): Promise<void>;

  getAuditRecords(query: {
    customerId: string;
    requestId?: string;
  }): Promise<ConfigStorePortAuditRecord[]>;
}
