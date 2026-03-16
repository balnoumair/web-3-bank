import type {
  CrePolicyConfigSnapshot,
  CrePolicyConfigUpsertRequest,
  CrePolicyScenarioUpdateRequest,
} from "../../src";

export const VALID_CONFIG_UPSERT_REQUEST: CrePolicyConfigUpsertRequest = {
  requestId: "req-25-config-001",
  customerId: "customer-25",
  allowedChains: ["base-sepolia", "arbitrum-sepolia"],
  active: true,
  activationThreshold: 0.7,
};

export const VALID_SCENARIO_UPDATE_REQUEST: CrePolicyScenarioUpdateRequest = {
  requestId: "req-25-scenario-001",
  customerId: "customer-25",
  scenario: "congested",
};

export const VALID_CONFIG_SNAPSHOT: CrePolicyConfigSnapshot = {
  customerId: "customer-25",
  configVersion: 3,
  updatedAt: "2026-02-21T20:00:00.000Z",
  allowedChains: ["base-sepolia", "arbitrum-sepolia"],
  active: true,
  scenario: "normal",
  activationThreshold: 0.7,
};
