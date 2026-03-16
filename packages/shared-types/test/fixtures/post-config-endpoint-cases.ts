import type { CrePolicyConfigUpsertRequest } from "../../src";

export const POST_CONFIG_ENDPOINT_CASES = {
  validRequest: {
    requestId: "req-29-valid-001",
    customerId: "customer-29",
    allowedChains: ["base-sepolia", "arbitrum-sepolia"],
    active: true,
    activationThreshold: 0.7,
  } satisfies CrePolicyConfigUpsertRequest,
  unsupportedChainRequest: {
    requestId: "req-29-unsupported-001",
    customerId: "customer-29",
    allowedChains: ["base-sepolia", "polygon-amoy"],
    active: true,
    activationThreshold: 0.7,
  } satisfies CrePolicyConfigUpsertRequest,
  supportedChains: ["base-sepolia", "arbitrum-sepolia", "optimism-sepolia"],
} as const;
