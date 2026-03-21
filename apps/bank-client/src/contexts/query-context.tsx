import { QueryClient, QueryClientProvider } from '@tanstack/solid-query';
import type { ParentComponent } from 'solid-js';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5_000,
      retry: 1,
    },
  },
});

export { queryClient };

export const QueryProvider: ParentComponent = (props) => {
  return (
    <QueryClientProvider client={queryClient}>
      {props.children}
    </QueryClientProvider>
  );
};
