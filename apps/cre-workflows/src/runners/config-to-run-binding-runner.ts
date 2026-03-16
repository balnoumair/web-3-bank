import type {
  ConfigStorePort,
  ConfigStorePortAuditRecord,
  CrePolicyConfigSnapshot,
  ScenarioName,
  SupportedChainRegistry,
} from "@repo/shared-types";
import {
  createPostConfigEndpoint,
  createPostScenarioEndpoint,
} from "@repo/shared-types";

import { bindRunStartConfigSnapshot } from "../run-start-snapshot-contract";
import { addMilliseconds, buildSequenceId } from "../workflow-utils";

type CrePolicyToEvaluatorNextRunValidationInput = {
  customerId: string;
  allowedChains: string[];
  active: boolean;
  scenario: ScenarioName;
  activationThreshold?: number;
};

type CrePolicyToEvaluatorNextRunValidationEvidence = {
  runId: string;
  requestId: string;
  configVersion: number;
  snapshot: CrePolicyConfigSnapshot;
  auditRecord: ConfigStorePortAuditRecord;
  auditTrail: ConfigStorePortAuditRecord[];
  correlation: {
    requestId: string;
    configVersion: number;
    runId: string;
  };
};

type CreateCrePolicyToEvaluatorNextRunValidationRunnerArgs = {
  store: ConfigStorePort;
  registry: SupportedChainRegistry;
  authorize: (authorizationHeader: string | undefined) => Promise<boolean> | boolean;
  runIdPrefix?: string;
  requestIdPrefix?: string;
  baseTimestamp?: string;
};



export function createCrePolicyToEvaluatorNextRunValidationRunner(
  args: CreateCrePolicyToEvaluatorNextRunValidationRunnerArgs,
): {
  runValidation: (
    input: CrePolicyToEvaluatorNextRunValidationInput,
  ) => Promise<CrePolicyToEvaluatorNextRunValidationEvidence>;
} {
  const runIdPrefix = args.runIdPrefix ?? "run-dev-26";
  const requestIdPrefix = args.requestIdPrefix ?? "req-dev-26";
  const baseTimestamp = args.baseTimestamp ?? "2026-02-22T00:00:00.000Z";

  const postConfig = createPostConfigEndpoint({
    store: args.store,
    registry: args.registry,
    authorize: args.authorize,
  });

  const postScenario = createPostScenarioEndpoint({
    store: args.store,
    authorize: args.authorize,
  });

  let sequence = 0;

  return {
    runValidation: async (input) => {
      sequence += 1;

      const runId = buildSequenceId(runIdPrefix, sequence);
      const configRequestId = `${requestIdPrefix}-config-${String(sequence).padStart(3, "0")}`;
      const scenarioRequestId = `${requestIdPrefix}-scenario-${String(sequence).padStart(3, "0")}`;

      const runOffset = (sequence - 1) * 60_000;
      const configAt = addMilliseconds(baseTimestamp, runOffset);
      const scenarioAt = addMilliseconds(configAt, 1_000);
      const runStartAt = addMilliseconds(scenarioAt, 1_000);

      const configResponse = await postConfig({
        authorizationHeader: "Bearer dev-26",
        now: configAt,
        payload: {
          requestId: configRequestId,
          customerId: input.customerId,
          allowedChains: input.allowedChains,
          active: input.active,
          activationThreshold: input.activationThreshold ?? 0.7,
        },
      });

      if (configResponse.status !== 200) {
        throw new Error(
          `config write failed: ${configResponse.body.errorCode} ${configResponse.body.errorMessage}`,
        );
      }

      const scenarioResponse = await postScenario({
        authorizationHeader: "Bearer dev-26",
        now: scenarioAt,
        payload: {
          requestId: scenarioRequestId,
          customerId: input.customerId,
          scenario: input.scenario,
        },
      });

      if (scenarioResponse.status !== 200) {
        throw new Error(
          `scenario write failed: ${scenarioResponse.body.errorCode} ${scenarioResponse.body.errorMessage}`,
        );
      }

      const binding = await bindRunStartConfigSnapshot({
        runId,
        customerId: input.customerId,
        hasActiveRun: false,
        timestamp: runStartAt,
        readConfigSnapshot: (customerId) => args.store.getConfigSnapshot(customerId),
      });

      if (binding.status !== "started") {
        throw new Error(`run start was not accepted: ${binding.status}`);
      }

      const auditTrail = await args.store.getAuditRecords({
        customerId: input.customerId,
      });

      const auditRecord = auditTrail.find(
        (record) =>
          record.requestId === scenarioRequestId &&
          record.action === "scenario.update" &&
          record.status === "accepted" &&
          record.configVersion === binding.log.snapshotVersion,
      );

      if (!auditRecord || auditRecord.configVersion === null) {
        throw new Error("missing accepted scenario audit record for consumed snapshot");
      }

      return {
        runId,
        requestId: auditRecord.requestId,
        configVersion: auditRecord.configVersion,
        snapshot: binding.snapshot,
        auditRecord,
        auditTrail,
        correlation: {
          requestId: auditRecord.requestId,
          configVersion: auditRecord.configVersion,
          runId,
        },
      };
    },
  };
}
