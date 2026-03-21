import { createQuery } from '@tanstack/solid-query';
import { gql, getAuthToken } from '~/lib/graphql';
import {
  RECENT_TRANSFERS_QUERY,
  type RecentTransfersResponse,
} from '~/queries/transfers';

export function useTransfers(limit: number = 10) {
  return createQuery(() => ({
    queryKey: ['recentTransfers', limit],
    queryFn: () =>
      gql<RecentTransfersResponse>(RECENT_TRANSFERS_QUERY, { limit }),
    enabled: !!getAuthToken(),
    select: (data) => data.recentTransfers,
  }));
}
