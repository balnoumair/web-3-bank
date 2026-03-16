import {
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
  createPostConfigEndpoint,
  createPostScenarioEndpoint,
  InMemorySupportedChainRegistry,
} from "@repo/shared-types";
import { describe, expect, it } from "vitest";

import { bindRunStartConfigSnapshot } from "../src";
import { HANDOFF_AUTH_INTEGRATION_CASES } from "./fixtures/handoff-auth-integration-cases";

describe("handoff/auth integration semantics", () => {
  it("applies writes before run start to current run and after run start to next run", async () => {
    const store = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase(),
    });
    const registry = new InMemorySupportedChainRegistry(
      HANDOFF_AUTH_INTEGRATION_CASES.allowedChains,
    );
    const authorize = (header: string | undefined): boolean =>
      header === HANDOFF_AUTH_INTEGRATION_CASES.auth.activeHeader;

    const postConfig = createPostConfigEndpoint({
      store,
      registry,
      authorize,
    });
    const postScenario = createPostScenarioEndpoint({
      store,
      authorize,
    });

    const configWrite = await postConfig({
      authorizationHeader: HANDOFF_AUTH_INTEGRATION_CASES.auth.activeHeader,
      now: HANDOFF_AUTH_INTEGRATION_CASES.timeline.configAt,
      payload: {
        requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.config,
        customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
        allowedChains: HANDOFF_AUTH_INTEGRATION_CASES.allowedChains,
        active: true,
        activationThreshold: 0.7,
      },
    });
    expect(configWrite.status).toBe(200);

    const beforeRunStartScenario = await postScenario({
      authorizationHeader: HANDOFF_AUTH_INTEGRATION_CASES.auth.activeHeader,
      now: HANDOFF_AUTH_INTEGRATION_CASES.timeline.beforeRunStartScenarioAt,
      payload: {
        requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.beforeRunStartScenario,
        customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
        scenario: "congested",
      },
    });
    expect(beforeRunStartScenario.status).toBe(200);

    const firstRun = await bindRunStartConfigSnapshot({
      runId: "run-43-001",
      customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
      hasActiveRun: false,
      timestamp: HANDOFF_AUTH_INTEGRATION_CASES.timeline.runStartAt,
      readConfigSnapshot: (customerId) => store.getConfigSnapshot(customerId),
    });

    if (firstRun.status !== "started") {
      throw new Error("expected first run to start");
    }

    const afterRunStartScenario = await postScenario({
      authorizationHeader: HANDOFF_AUTH_INTEGRATION_CASES.auth.activeHeader,
      now: HANDOFF_AUTH_INTEGRATION_CASES.timeline.afterRunStartScenarioAt,
      payload: {
        requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.afterRunStartScenario,
        customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
        scenario: "normal",
      },
    });
    expect(afterRunStartScenario.status).toBe(200);

    expect(firstRun.snapshot.configVersion).toBe(2);
    expect(firstRun.snapshot.scenario).toBe("congested");

    const secondRun = await bindRunStartConfigSnapshot({
      runId: "run-43-002",
      customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
      hasActiveRun: false,
      timestamp: HANDOFF_AUTH_INTEGRATION_CASES.timeline.nextRunStartAt,
      readConfigSnapshot: (customerId) => store.getConfigSnapshot(customerId),
    });

    if (secondRun.status !== "started") {
      throw new Error("expected second run to start");
    }

    expect(secondRun.snapshot.configVersion).toBe(3);
    expect(secondRun.snapshot.scenario).toBe("normal");
  });

  it("returns UNAUTHORIZED for revoked or missing API keys and audits failures", async () => {
    const store = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase(),
    });
    const registry = new InMemorySupportedChainRegistry(
      HANDOFF_AUTH_INTEGRATION_CASES.allowedChains,
    );
    const authorize = (header: string | undefined): boolean =>
      header === HANDOFF_AUTH_INTEGRATION_CASES.auth.activeHeader;

    const postConfig = createPostConfigEndpoint({
      store,
      registry,
      authorize,
    });
    const postScenario = createPostScenarioEndpoint({
      store,
      authorize,
    });

    const unauthorizedConfig = await postConfig({
      authorizationHeader: HANDOFF_AUTH_INTEGRATION_CASES.auth.revokedHeader,
      now: HANDOFF_AUTH_INTEGRATION_CASES.timeline.configAt,
      payload: {
        requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.unauthorizedConfig,
        customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
        allowedChains: HANDOFF_AUTH_INTEGRATION_CASES.allowedChains,
        active: true,
        activationThreshold: 0.7,
      },
    });

    const unauthorizedScenario = await postScenario({
      authorizationHeader: undefined,
      now: HANDOFF_AUTH_INTEGRATION_CASES.timeline.runStartAt,
      payload: {
        requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.unauthorizedScenario,
        customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
        scenario: "normal",
      },
    });

    expect(unauthorizedConfig).toEqual({
      status: 401,
      body: {
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.unauthorizedConfig,
      },
    });
    expect(unauthorizedScenario).toEqual({
      status: 401,
      body: {
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.unauthorizedScenario,
      },
    });

    const configAudit = await store.getAuditRecords({
      customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
      requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.unauthorizedConfig,
    });
    const scenarioAudit = await store.getAuditRecords({
      customerId: HANDOFF_AUTH_INTEGRATION_CASES.customerId,
      requestId: HANDOFF_AUTH_INTEGRATION_CASES.requestIds.unauthorizedScenario,
    });

    expect(configAudit[0]).toMatchObject({
      status: "rejected",
      errorCode: "UNAUTHORIZED",
      configVersion: null,
    });
    expect(scenarioAudit[0]).toMatchObject({
      status: "rejected",
      errorCode: "UNAUTHORIZED",
      configVersion: null,
    });
  });
});
