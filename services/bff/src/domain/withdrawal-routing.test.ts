import { describe, expect, it } from "bun:test";
import {
  selectWithdrawalChain,
  sumUnavailableBalanceWei,
  type WithdrawalRoutingEntry,
} from "./withdrawal-routing.js";

function entry(
  chainId: string,
  withdrawableWei: string,
  available: boolean,
  balanceWei: string,
  reason = "",
): WithdrawalRoutingEntry {
  return { chainId, withdrawableWei, available, balanceWei, reason };
}

describe("selectWithdrawalChain", () => {
  it("uses preferred chain when it can satisfy the amount", () => {
    const result = selectWithdrawalChain(
      [
        entry("84532", "2000000", true, "2000000"),
        entry("42161", "5000000", true, "5000000"),
      ],
      1_000_000n,
      "84532",
    );
    expect(result).toEqual({
      ok: true,
      chainId: "84532",
      withdrawableWei: "2000000",
    });
  });

  it("falls back to another healthy chain when preferred cannot satisfy amount", () => {
    const result = selectWithdrawalChain(
      [
        entry("84532", "500000", true, "500000"),
        entry("42161", "5000000", true, "5000000"),
      ],
      1_000_000n,
      "84532",
    );
    expect(result).toEqual({
      ok: true,
      chainId: "42161",
      withdrawableWei: "5000000",
    });
  });

  it("reports unavailable when only inactive chain has balance", () => {
    const result = selectWithdrawalChain(
      [entry("42161", "1000000", false, "1000000", "chain_inactive")],
      500_000n,
      "84532",
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toContain("temporarily unavailable");
    }
  });

  it("reports reserve cap when amount exceeds any healthy chain withdrawable", () => {
    const result = selectWithdrawalChain(
      [entry("84532", "500000", true, "2000000")],
      1_000_000n,
      "84532",
    );
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toContain("Insufficient");
    }
  });
});

describe("sumUnavailableBalanceWei", () => {
  it("sums balance on inactive chains only", () => {
    expect(
      sumUnavailableBalanceWei([
        entry("42161", "1000000", false, "1000000", "chain_inactive"),
        entry("84532", "2000000", true, "2000000"),
      ]),
    ).toBe(1_000_000n);
  });
});
