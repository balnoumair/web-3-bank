import type { CrePolicyScenarioUpdateRequest } from "../../src";

export const POST_SCENARIO_ENDPOINT_CASES = {
  validRequest: {
    requestId: "req-30-valid-001",
    customerId: "customer-30",
    scenario: "congested",
  } satisfies CrePolicyScenarioUpdateRequest,
  invalidScenarioRequest: {
    requestId: "req-30-invalid-scenario",
    customerId: "customer-30",
    scenario: "degraded",
  },
} as const;
