import { createQuery } from '@tanstack/solid-query';
import { gql, getAuthToken } from '~/lib/graphql';
import { ME_QUERY, type MeResponse } from '~/queries/auth';

export function useUser() {
  return createQuery(() => ({
    queryKey: ['me'],
    queryFn: () => gql<MeResponse>(ME_QUERY),
    enabled: !!getAuthToken(),
    select: (data) => data.me,
  }));
}
