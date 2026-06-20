/**
 * Integration-style test for withdrawal failover semantics:
 * two chains, one inactive — routing exposes unavailable balance and
 * chain selection prefers the healthy chain.
 */
import { describe, expect, it } from "bun:test";
import {
  selectWithdrawalChain,
  sumUnavailableBalanceWei,
  type WithdrawalRoutingEntry,
} from "../domain/withdrawal-routing.js";

describe("withdrawal failover E2E semantics", () => {
  const twoChainRouting: WithdrawalRoutingEntry[] = [
    {
      chainId: "42161",
      balanceWei: "1500000000",
      withdrawableWei: "1500000000",
      available: false,
      reason: "chain_inactive",
    },
    {
      chainId: "84532",
      balanceWei: "500000000",
      withdrawableWei: "500000000",
      available: true,
      reason: "",
    },
  ];

  it("reports inactive-chain balance as unavailable", () => {
    expect(sumUnavailableBalanceWei(twoChainRouting)).toBe(1_500_000_000n);
  });

  it("routes withdrawal to the healthy chain when amount fits", () => {
    const result = selectWithdrawalChain(twoChainRouting, 200_000_000n, "42161");
    expect(result).toEqual({
      ok: true,
      chainId: "84532",
      withdrawableWei: "500000000",
    });
  });

  it("blocks withdrawal when only inactive chain holds balance", () => {
    const onlyInactive: WithdrawalRoutingEntry[] = [twoChainRouting[0]!];
    const result = selectWithdrawalChain(onlyInactive, 100_000_000n, "42161");
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toContain("temporarily unavailable");
    }
  });
});
