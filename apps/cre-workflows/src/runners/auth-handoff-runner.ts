import type {
  ConfigStorePort,
  ConfigStorePortAuditRecord,
  CrePolicyConfigSnapshot,
  PostConfigEndpointResponse,
  ScenarioName,
  SupportedChainRegistry,
} from "@repo/shared-types";
import {
  createPostConfigEndpoint,
  createPostScenarioEndpoint,
} from "@repo/shared-types";

import { bindRunStartConfigSnapshot } from "../run-start-snapshot-contract";
import { addMilliseconds, buildSequenceId } from "../workflow-utils";

type HandoffAuthE2eInput = {
  customerId: string;
  allowedChains: string[];
  active: boolean;
  scenario: ScenarioName;
  activationThreshold?: number;
};

type HandoffAuthE2eEvidence = {
  accepted: {
    runId: string;
    requestId: string;
    configVersion: number;
    snapshot: CrePolicyConfigSnapshot;
    auditRecord: ConfigStorePortAuditRecord;
  };
  rejected: {
    requestId: string;
    response: PostConfigEndpointResponse;
    auditRecord: ConfigStorePortAuditRecord;
  };
  linkage: {
    requestId: string;
    configVersion: number;
    runId: string;
  };
  auditTrail: ConfigStorePortAuditRecord[];
};

type CreateHandoffAuthE2eRunnerArgs = {
  store: ConfigStorePort;
  registry: SupportedChainRegistry;
  authorize: (authorizationHeader: string | undefined) => Promise<boolean> | boolean;
  activeAuthorizationHeader: string;
  revokedAuthorizationHeader: string;
  runIdPrefix?: string;
  requestIdPrefix?: string;
  baseTimestamp?: string;
};



export function createHandoffAuthE2eRunner(args: CreateHandoffAuthE2eRunnerArgs): {
  runValidation: (input: HandoffAuthE2eInput) => Promise<HandoffAuthE2eEvidence>;
} {
  const runIdPrefix = args.runIdPrefix ?? "run-dev-39";
  const requestIdPrefix = args.requestIdPrefix ?? "req-dev-39";
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
      const rejectedConfigRequestId =
        `${requestIdPrefix}-config-rejected-${String(sequence).padStart(3, "0")}`;

      const runOffset = (sequence - 1) * 60_000;
      const configAt = addMilliseconds(baseTimestamp, runOffset);
      const scenarioAt = addMilliseconds(configAt, 1_000);
      const runStartAt = addMilliseconds(scenarioAt, 1_000);
      const rejectedAt = addMilliseconds(runStartAt, 1_000);

      const configResponse = await postConfig({
        authorizationHeader: args.activeAuthorizationHeader,
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
        authorizationHeader: args.activeAuthorizationHeader,
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

      const rejectedResponse = await postConfig({
        authorizationHeader: args.revokedAuthorizationHeader,
        now: rejectedAt,
        payload: {
          requestId: rejectedConfigRequestId,
          customerId: input.customerId,
          allowedChains: input.allowedChains,
          active: input.active,
          activationThreshold: input.activationThreshold ?? 0.7,
        },
      });

      const auditTrail = await args.store.getAuditRecords({
        customerId: input.customerId,
      });

      const acceptedAuditRecord = auditTrail.find(
        (record) =>
          record.requestId === scenarioRequestId &&
          record.action === "scenario.update" &&
          record.status === "accepted" &&
          record.configVersion === binding.snapshot.configVersion,
      );

      if (!acceptedAuditRecord || acceptedAuditRecord.configVersion === null) {
        throw new Error("missing accepted scenario audit record for consumed snapshot");
      }

      const rejectedAuditRecord = auditTrail.find(
        (record) =>
          record.requestId === rejectedConfigRequestId &&
          record.action === "config.upsert" &&
          record.status === "rejected" &&
          record.errorCode === "UNAUTHORIZED",
      );

      if (!rejectedAuditRecord) {
        throw new Error("missing rejected config audit record for unauthorized write");
      }

      return {
        accepted: {
          runId,
          requestId: acceptedAuditRecord.requestId,
          configVersion: acceptedAuditRecord.configVersion,
          snapshot: binding.snapshot,
          auditRecord: acceptedAuditRecord,
        },
        rejected: {
          requestId: rejectedConfigRequestId,
          response: rejectedResponse,
          auditRecord: rejectedAuditRecord,
        },
        linkage: {
          requestId: acceptedAuditRecord.requestId,
          configVersion: acceptedAuditRecord.configVersion,
          runId,
        },
        auditTrail,
      };
    },
  };
}
