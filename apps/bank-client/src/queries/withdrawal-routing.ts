export const WITHDRAWAL_ROUTING_QUERY = `
  query WithdrawalRouting {
    withdrawalRouting {
      chainId
      withdrawableWei
      available
      reason
      balanceWei
    }
  }
`;

export interface WithdrawalRoutingEntry {
  chainId: string;
  withdrawableWei: string;
  available: boolean;
  reason?: string | null;
  balanceWei: string;
}

export interface WithdrawalRoutingResponse {
  withdrawalRouting: WithdrawalRoutingEntry[];
}

/**
 * Pick the best chain to withdraw on: prefer the local chain when it can
 * cover the amount, otherwise the first available chain with enough balance.
 */
export function selectWithdrawalChain(
  entries: WithdrawalRoutingEntry[],
  amountWei: bigint,
  preferredChainId: number,
): { chainId: string; withdrawableWei: bigint } | null {
  const preferred = String(preferredChainId);
  const sorted = [
    ...entries.filter((e) => e.chainId === preferred),
    ...entries.filter((e) => e.chainId !== preferred),
  ];

  for (const entry of sorted) {
    if (!entry.available) continue;
    const withdrawable = BigInt(entry.withdrawableWei);
    if (withdrawable >= amountWei) {
      return { chainId: entry.chainId, withdrawableWei: withdrawable };
    }
  }
  return null;
}

/**
 * Human-readable message when no chain can fulfill the requested amount.
 */
export function withdrawalUnavailableMessage(
  entries: WithdrawalRoutingEntry[],
): string {
  const inactive = entries.find((e) => !e.available && e.reason === 'chain_inactive');
  if (inactive) {
    return 'Withdrawal temporarily unavailable — your balance is on a chain that is not processing transactions right now.';
  }
  const capped = entries.find((e) => e.available && BigInt(e.withdrawableWei) > 0n);
  if (capped) {
    return `Withdrawal amount exceeds the currently available reserve on active chains (max ${capped.withdrawableWei} wei).`;
  }
  return 'Withdrawal temporarily unavailable — no active chain can process your request right now.';
}

/** Sum SyncUSD balance stranded on inactive chains (shown as a separate dashboard line). */
export function unavailableWithdrawalWei(entries: WithdrawalRoutingEntry[]): bigint {
  return entries.reduce((total, entry) => {
    if (!entry.available && entry.reason === 'chain_inactive') {
      return total + BigInt(entry.balanceWei);
    }
    return total;
  }, 0n);
}
