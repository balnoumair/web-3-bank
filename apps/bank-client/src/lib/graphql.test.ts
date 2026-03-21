import { describe, it, expect, vi, beforeEach } from 'vitest';
import { GraphQLError, getAuthToken, setAuthToken, gql } from './graphql';

describe('GraphQLError', () => {
  it('is an instance of Error', () => {
    const err = new GraphQLError('something failed', [{ message: 'something failed' }]);
    expect(err).toBeInstanceOf(Error);
  });

  it('has name "GraphQLError"', () => {
    const err = new GraphQLError('oops', []);
    expect(err.name).toBe('GraphQLError');
  });

  it('exposes message and errors array', () => {
    const errors = [{ message: 'Unauthorized' }, { message: 'Not found' }];
    const err = new GraphQLError('Unauthorized', errors);
    expect(err.message).toBe('Unauthorized');
    expect(err.errors).toEqual(errors);
  });
});

describe('getAuthToken / setAuthToken', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('returns null when no token is stored', () => {
    expect(getAuthToken()).toBeNull();
  });

  it('stores and retrieves a token', () => {
    setAuthToken('abc.def.ghi');
    expect(getAuthToken()).toBe('abc.def.ghi');
  });

  it('overwrites an existing token', () => {
    setAuthToken('first');
    setAuthToken('second');
    expect(getAuthToken()).toBe('second');
  });

  it('removes the token when null is passed', () => {
    setAuthToken('abc.def.ghi');
    setAuthToken(null);
    expect(getAuthToken()).toBeNull();
  });
});

describe('gql', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
  });

  it('makes a POST request and returns data on success', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValueOnce({
        json: () => Promise.resolve({ data: { balance: '1000000' } }),
      }),
    );

    const result = await gql<{ balance: string }>('{ balance }');
    expect(result).toEqual({ balance: '1000000' });
  });

  it('sends Content-Type: application/json', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce({
      json: () => Promise.resolve({ data: {} }),
    });
    vi.stubGlobal('fetch', fetchMock);

    await gql('{ balance }');

    const [, options] = fetchMock.mock.calls[0];
    expect(options.headers['Content-Type']).toBe('application/json');
  });

  it('includes Authorization header when token is set', async () => {
    setAuthToken('my-jwt-token');
    const fetchMock = vi.fn().mockResolvedValueOnce({
      json: () => Promise.resolve({ data: {} }),
    });
    vi.stubGlobal('fetch', fetchMock);

    await gql('{ balance }');

    const [, options] = fetchMock.mock.calls[0];
    expect(options.headers['Authorization']).toBe('Bearer my-jwt-token');
  });

  it('omits Authorization header when no token', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce({
      json: () => Promise.resolve({ data: {} }),
    });
    vi.stubGlobal('fetch', fetchMock);

    await gql('{ balance }');

    const [, options] = fetchMock.mock.calls[0];
    expect(options.headers['Authorization']).toBeUndefined();
  });

  it('sends variables in the request body', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce({
      json: () => Promise.resolve({ data: {} }),
    });
    vi.stubGlobal('fetch', fetchMock);

    const vars = { id: '42' };
    await gql('query ($id: ID!) { user(id: $id) { id } }', vars);

    const [, options] = fetchMock.mock.calls[0];
    const body = JSON.parse(options.body);
    expect(body.variables).toEqual(vars);
  });

  it('throws GraphQLError when the response contains errors', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        json: () =>
          Promise.resolve({ errors: [{ message: 'Unauthorized' }] }),
      }),
    );

    await expect(gql('{ balance }')).rejects.toThrow(GraphQLError);
    await expect(gql('{ balance }')).rejects.toThrow('Unauthorized');
  });

  it('uses the first error message', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValueOnce({
        json: () =>
          Promise.resolve({
            errors: [{ message: 'First error' }, { message: 'Second error' }],
          }),
      }),
    );

    await expect(gql('{ balance }')).rejects.toThrow('First error');
  });
});
