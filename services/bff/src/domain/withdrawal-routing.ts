/**
 * Pure policy for picking which chain to execute a withdrawal on.
 * Prefers the user's connected chain when it can satisfy the amount.
 */

export type WithdrawalRoutingEntry = {
  chainId: string;
  withdrawableWei: string;
  available: boolean;
  reason: string;
  balanceWei: string;
};

export type WithdrawalSelection =
  | { ok: true; chainId: string; withdrawableWei: string }
  | { ok: false; reason: string };

export function selectWithdrawalChain(
  entries: WithdrawalRoutingEntry[],
  amountWei: bigint,
  preferredChainId: string,
): WithdrawalSelection {
  const viable = entries.filter((e) => {
    if (!e.available) return false;
    try {
      return BigInt(e.withdrawableWei) >= amountWei;
    } catch {
      return false;
    }
  });

  if (viable.length === 0) {
    const unavailable = entries.filter((e) => !e.available);
    if (unavailable.length > 0 && entries.every((e) => !e.available)) {
      return {
        ok: false,
        reason:
          unavailable[0]?.reason === "chain_inactive"
            ? "Withdrawal temporarily unavailable — your balance is on a chain that cannot process transactions right now."
            : "Withdrawal temporarily unavailable.",
      };
    }

    const maxEntry = entries
      .filter((e) => e.available)
      .sort((a, b) => {
        const av = BigInt(a.withdrawableWei);
        const bv = BigInt(b.withdrawableWei);
        return av === bv ? 0 : av > bv ? -1 : 1;
      })[0];

    if (maxEntry) {
      return {
        ok: false,
        reason: `Insufficient withdrawable balance. Maximum on any healthy chain: ${maxEntry.withdrawableWei} wei.`,
      };
    }

    return { ok: false, reason: "No withdrawable balance available." };
  }

  const preferred = viable.find((e) => e.chainId === preferredChainId);
  const chosen = preferred ?? viable[0]!;
  return {
    ok: true,
    chainId: chosen.chainId,
    withdrawableWei: chosen.withdrawableWei,
  };
}

/** Sum balance on chains marked unavailable (e.g. inactive). */
export function sumUnavailableBalanceWei(
  entries: WithdrawalRoutingEntry[],
): bigint {
  let total = 0n;
  for (const e of entries) {
    if (!e.available) {
      try {
        total += BigInt(e.balanceWei);
      } catch {
        // skip malformed
      }
    }
  }
  return total;
}
