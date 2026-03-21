import { env } from '~/config/env';

export class GraphQLError extends Error {
  constructor(
    message: string,
    public errors: Array<{ message: string }>,
  ) {
    super(message);
    this.name = 'GraphQLError';
  }
}

export function getAuthToken(): string | null {
  if (typeof window === 'undefined') return null;
  return localStorage.getItem('auth_token');
}

export function setAuthToken(token: string | null): void {
  if (typeof window === 'undefined') return;
  if (token) {
    localStorage.setItem('auth_token', token);
  } else {
    localStorage.removeItem('auth_token');
  }
}

export async function gql<T>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<T> {
  const token = getAuthToken();

  const res = await fetch(env.bffUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify({ query, variables }),
  });

  const json = await res.json();

  if (json.errors?.length) {
    throw new GraphQLError(json.errors[0].message, json.errors);
  }

  return json.data as T;
}
