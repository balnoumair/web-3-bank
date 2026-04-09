import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { gql } from '~/lib/graphql';
import { SET_USERNAME_MUTATION, type SetUsernameResponse } from '~/queries/auth';

export function useSetUsername() {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async (username: string) => {
      const data = await gql<SetUsernameResponse>(SET_USERNAME_MUTATION, { username });
      return data.setUsername;
    },
    onSuccess: () => {
      // Refresh the user profile so the new username shows everywhere.
      queryClient.invalidateQueries({ queryKey: ['me'] });
    },
  }));
}
