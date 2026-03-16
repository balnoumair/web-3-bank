export const HANDOFF_AUTH_INTEGRATION_CASES = {
  customerId: "customer-43",
  allowedChains: ["base-sepolia", "arbitrum-sepolia"],
  timeline: {
    configAt: "2026-02-22T14:00:00.000Z",
    beforeRunStartScenarioAt: "2026-02-22T14:00:00.999Z",
    runStartAt: "2026-02-22T14:00:01.000Z",
    afterRunStartScenarioAt: "2026-02-22T14:00:01.001Z",
    nextRunStartAt: "2026-02-22T14:01:01.000Z",
  },
  requestIds: {
    config: "req-43-config-001",
    beforeRunStartScenario: "req-43-scenario-001",
    afterRunStartScenario: "req-43-scenario-002",
    unauthorizedConfig: "req-43-config-unauthorized",
    unauthorizedScenario: "req-43-scenario-unauthorized",
  },
  auth: {
    activeHeader: "Bearer key-43-active",
    revokedHeader: "Bearer key-43-revoked",
  },
} as const;
