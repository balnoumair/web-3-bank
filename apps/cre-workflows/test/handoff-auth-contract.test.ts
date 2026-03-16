import {
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
  createPostConfigEndpoint,
  createPostScenarioEndpoint,
  InMemorySupportedChainRegistry,
} from "@repo/shared-types";
import { describe, expect, it } from "vitest";

import { bindRunStartConfigSnapshot } from "../src";
import { HANDOFF_AUTH_CONTRACT_CASES } from "./fixtures/handoff-auth-contract-cases";

describe("handoff and auth contract", () => {
  it("pins run-start snapshot and applies post-start writes to next run", async () => {
    const store = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase(),
    });
    const registry = new InMemorySupportedChainRegistry(
      HANDOFF_AUTH_CONTRACT_CASES.supportedChains,
    );

    const postConfig = createPostConfigEndpoint({
      store,
      registry,
      authorize: () => true,
    });
    const postScenario = createPostScenarioEndpoint({
      store,
      authorize: () => true,
    });

    const seed = HANDOFF_AUTH_CONTRACT_CASES.pinnedSnapshotBoundary;

    const configResponse = await postConfig({
      authorizationHeader: "Bearer key-38",
      now: seed.configAt,
      payload: {
        requestId: seed.initialConfigRequestId,
        customerId: seed.customerId,
        allowedChains: seed.initialAllowedChains,
        active: true,
        activationThreshold: 0.7,
      },
    });
    expect(configResponse.status).toBe(200);

    const scenarioResponse = await postScenario({
      authorizationHeader: "Bearer key-38",
      now: seed.scenarioAt,
      payload: {
        requestId: seed.initialScenarioRequestId,
        customerId: seed.customerId,
        scenario: seed.initialScenario,
      },
    });
    expect(scenarioResponse.status).toBe(200);

    const firstRunBinding = await bindRunStartConfigSnapshot({
      runId: "run-38-001",
      customerId: seed.customerId,
      hasActiveRun: false,
      timestamp: seed.runStartAt,
      readConfigSnapshot: (customerId) => store.getConfigSnapshot(customerId),
    });

    if (firstRunBinding.status !== "started") {
      throw new Error("expected first run to start");
    }

    const midRunWriteResponse = await postScenario({
      authorizationHeader: "Bearer key-38",
      now: seed.postRunStartWriteAt,
      payload: {
        requestId: seed.postRunStartScenarioRequestId,
        customerId: seed.customerId,
        scenario: seed.postRunStartScenario,
      },
    });
    expect(midRunWriteResponse.status).toBe(200);

    expect(firstRunBinding.snapshot.configVersion).toBe(2);
    expect(firstRunBinding.snapshot.scenario).toBe("congested");

    const secondRunBinding = await bindRunStartConfigSnapshot({
      runId: "run-38-002",
      customerId: seed.customerId,
      hasActiveRun: false,
      timestamp: seed.nextRunStartAt,
      readConfigSnapshot: (customerId) => store.getConfigSnapshot(customerId),
    });

    if (secondRunBinding.status !== "started") {
      throw new Error("expected second run to start");
    }

    expect(secondRunBinding.snapshot.configVersion).toBe(3);
    expect(secondRunBinding.snapshot.scenario).toBe("normal");
  });

  it("audits rejected and accepted write outcomes for auth semantics", async () => {
    const store = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase(),
    });
    const registry = new InMemorySupportedChainRegistry(
      HANDOFF_AUTH_CONTRACT_CASES.supportedChains,
    );
    const auth = HANDOFF_AUTH_CONTRACT_CASES.authAuditParity;

    const endpoint = createPostConfigEndpoint({
      store,
      registry,
      authorize: (header) => header === "Bearer key-38-valid",
    });

    const unauthorized = await endpoint({
      authorizationHeader: "Bearer key-38-revoked",
      now: auth.unauthorizedAt,
      payload: {
        requestId: auth.unauthorizedRequestId,
        customerId: auth.customerId,
        allowedChains: auth.authorizedAllowedChains,
        active: true,
        activationThreshold: 0.7,
      },
    });

    const authorized = await endpoint({
      authorizationHeader: "Bearer key-38-valid",
      now: auth.authorizedAt,
      payload: {
        requestId: auth.authorizedRequestId,
        customerId: auth.customerId,
        allowedChains: auth.authorizedAllowedChains,
        active: true,
        activationThreshold: 0.7,
      },
    });

    expect(unauthorized).toEqual({
      status: 401,
      body: {
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId: auth.unauthorizedRequestId,
      },
    });
    expect(authorized.status).toBe(200);

    const rejectedAudit = await store.getAuditRecords({
      customerId: auth.customerId,
      requestId: auth.unauthorizedRequestId,
    });
    const acceptedAudit = await store.getAuditRecords({
      customerId: auth.customerId,
      requestId: auth.authorizedRequestId,
    });

    expect(rejectedAudit[0]).toMatchObject({
      requestId: auth.unauthorizedRequestId,
      status: "rejected",
      errorCode: "UNAUTHORIZED",
      configVersion: null,
    });
    expect(acceptedAudit[0]).toMatchObject({
      requestId: auth.authorizedRequestId,
      status: "accepted",
      errorCode: null,
      configVersion: 1,
    });
  });
});
