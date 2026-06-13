import { createQuery } from '@tanstack/solid-query';
import { gql, getAuthToken } from '~/lib/graphql';
import {
  unavailableWithdrawalWei,
  WITHDRAWAL_ROUTING_QUERY,
  type WithdrawalRoutingResponse,
} from '~/queries/withdrawal-routing';

export function useWithdrawalRouting() {
  return createQuery(() => ({
    queryKey: ['withdrawalRouting'],
    queryFn: () => gql<WithdrawalRoutingResponse>(WITHDRAWAL_ROUTING_QUERY),
    enabled: !!getAuthToken(),
    refetchInterval: 10_000,
    select: (data) => ({
      entries: data.withdrawalRouting,
      unavailableWei: unavailableWithdrawalWei(data.withdrawalRouting),
    }),
  }));
}
