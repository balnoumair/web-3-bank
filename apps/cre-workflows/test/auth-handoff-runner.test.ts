import {
  createConfigStoreAdapter,
  createInMemoryConfigStoreDatabase,
  InMemorySupportedChainRegistry,
} from "@repo/shared-types";
import { describe, expect, it } from "vitest";

import { createHandoffAuthE2eRunner } from "../src";
import { HANDOFF_AUTH_E2E_CASE } from "./fixtures/auth-handoff-runner-case";

describe("handoff/auth e2e validation", () => {
  it("produces accepted and rejected evidence linked by requestId, configVersion, and runId", async () => {
    const store = createConfigStoreAdapter({
      db: createInMemoryConfigStoreDatabase(),
    });
    const registry = new InMemorySupportedChainRegistry(
      HANDOFF_AUTH_E2E_CASE.allowedChains,
    );

    const runner = createHandoffAuthE2eRunner({
      store,
      registry,
      authorize: (header) => header === HANDOFF_AUTH_E2E_CASE.headers.active,
      activeAuthorizationHeader: HANDOFF_AUTH_E2E_CASE.headers.active,
      revokedAuthorizationHeader: HANDOFF_AUTH_E2E_CASE.headers.revoked,
      runIdPrefix: "run-39",
      requestIdPrefix: "req-39",
      baseTimestamp: HANDOFF_AUTH_E2E_CASE.baseTimestamp,
    });

    const evidence = await runner.runValidation({
      customerId: HANDOFF_AUTH_E2E_CASE.customerId,
      allowedChains: [...HANDOFF_AUTH_E2E_CASE.allowedChains],
      active: true,
      scenario: HANDOFF_AUTH_E2E_CASE.scenario,
    });

    expect(evidence.accepted.runId).toBe("run-39-001");
    expect(evidence.accepted.requestId).toBe("req-39-scenario-001");
    expect(evidence.accepted.snapshot.configVersion).toBe(evidence.accepted.configVersion);
    expect(evidence.accepted.auditRecord.status).toBe("accepted");

    expect(evidence.rejected.requestId).toBe("req-39-config-rejected-001");
    expect(evidence.rejected.response.status).toBe(401);
    if (evidence.rejected.response.status !== 401) {
      throw new Error("expected rejected response to be unauthorized");
    }
    expect(evidence.rejected.response.body.errorCode).toBe("UNAUTHORIZED");
    expect(evidence.rejected.auditRecord.status).toBe("rejected");
    expect(evidence.rejected.auditRecord.errorCode).toBe("UNAUTHORIZED");

    expect(evidence.linkage).toEqual({
      requestId: evidence.accepted.requestId,
      configVersion: evidence.accepted.configVersion,
      runId: evidence.accepted.runId,
    });
  });
});
