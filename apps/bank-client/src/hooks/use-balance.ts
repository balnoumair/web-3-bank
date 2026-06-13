import { createQuery } from '@tanstack/solid-query';
import { gql, getAuthToken } from '~/lib/graphql';
import { BALANCE_QUERY, type BalanceResponse } from '~/queries/balance';

export function useBalance() {
  return createQuery(() => ({
    queryKey: ['balance'],
    queryFn: () => gql<BalanceResponse>(BALANCE_QUERY),
    enabled: !!getAuthToken(),
    refetchInterval: 10_000,
    select: (data) => data.balance.amountWei,
  }));
}
