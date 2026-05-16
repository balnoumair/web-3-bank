import { describe, expect, it } from "bun:test";
import { resolveDestChainId } from "./send-routing.js";

describe("resolveDestChainId", () => {
  it("uses sender chain when no home chain", () => {
    expect(
      resolveDestChainId({
        senderChainId: 84532n,
        recipientHomeChainId: undefined,
        recipientHomeChainActive: true,
        recipientHomeChainDecommissioned: false,
      }),
    ).toBe(84532n);
  });

  it("uses home chain when present and active", () => {
    expect(
      resolveDestChainId({
        senderChainId: 84532n,
        recipientHomeChainId: 42161n,
        recipientHomeChainActive: true,
        recipientHomeChainDecommissioned: false,
      }),
    ).toBe(42161n);
  });

  it("falls back to sender when home is inactive", () => {
    expect(
      resolveDestChainId({
        senderChainId: 84532n,
        recipientHomeChainId: 42161n,
        recipientHomeChainActive: false,
        recipientHomeChainDecommissioned: false,
      }),
    ).toBe(84532n);
  });

  it("falls back to sender when home is decommissioned even if active data is stale", () => {
    expect(
      resolveDestChainId({
        senderChainId: 84532n,
        recipientHomeChainId: 42161n,
        recipientHomeChainActive: true,
        recipientHomeChainDecommissioned: true,
      }),
    ).toBe(84532n);
  });
});
