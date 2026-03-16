export const HANDOFF_AUTH_E2E_CASE = {
  customerId: "customer-39",
  allowedChains: ["base-sepolia", "arbitrum-sepolia"],
  scenario: "congested",
  headers: {
    active: "Bearer key-39-active",
    revoked: "Bearer key-39-revoked",
  },
  baseTimestamp: "2026-02-22T16:00:00.000Z",
} as const;
