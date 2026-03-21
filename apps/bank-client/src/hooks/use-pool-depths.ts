import { createQuery } from '@tanstack/solid-query';
import { gql, getAuthToken } from '~/lib/graphql';
import { POOL_DEPTHS_QUERY, type PoolDepthsResponse } from '~/queries/pool';

export function usePoolDepths(chainId: number) {
  return createQuery(() => ({
    queryKey: ['poolDepths', chainId],
    queryFn: () =>
      gql<PoolDepthsResponse>(POOL_DEPTHS_QUERY, { chainId }),
    enabled: !!getAuthToken(),
    select: (data) => data.poolDepths,
  }));
}
