import { createSignal, onMount } from 'solid-js';
import { createPasskeyCredential, getPasskeyCredential } from '~/lib/passkey';
import { deriveTempoAddress } from '~/lib/address';
import { gql, setAuthToken, getAuthToken } from '~/lib/graphql';
import { queryClient } from '~/contexts/query-context';
import {
  ME_QUERY,
  REGISTER_USER_MUTATION,
  AUTHENTICATE_MUTATION,
  type User,
  type MeResponse,
  type RegisterUserResponse,
  type AuthenticateResponse,
} from '~/queries/auth';
import { env } from '~/config/env';

export function createAuth() {
  const [user, setUser] = createSignal<User | null>(null);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  // Restore session on mount
  onMount(async () => {
    const token = getAuthToken();
    if (token) {
      try {
        const data = await gql<MeResponse>(ME_QUERY);
        setUser(data.me);
      } catch {
        setAuthToken(null);
      }
    }
    setIsLoading(false);
  });

  const register = async (displayName: string) => {
    setIsLoading(true);
    setError(null);
    try {
      // 1. Create passkey (triggers biometric)
      const credential = await createPasskeyCredential(displayName);

      // 2. Derive Tempo address from P-256 public key
      const address = deriveTempoAddress(credential.publicKey);

      // 3. Encode public key as hex for BFF
      const publicKeyHex = Array.from(credential.publicKey)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');

      // 4. Register with BFF
      const data = await gql<RegisterUserResponse>(REGISTER_USER_MUTATION, {
        address,
        credentialId: credential.credentialId,
        publicKey: publicKeyHex,
        displayName,
        chainId: env.tempoChainId,
      });

      // 5. Store JWT
      setAuthToken(data.registerUser.token);

      // 6. Fetch full user profile
      const meData = await gql<MeResponse>(ME_QUERY);
      setUser(meData.me);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Registration failed';
      setError(message);
      throw err;
    } finally {
      setIsLoading(false);
    }
  };

  const login = async () => {
    setIsLoading(true);
    setError(null);
    try {
      // 1. Get existing passkey (triggers biometric)
      const assertion = await getPasskeyCredential();

      // 2. Authenticate — the BFF looks up the user by credentialId
      const data = await gql<AuthenticateResponse>(AUTHENTICATE_MUTATION, {
        credentialId: assertion.credentialId,
        chainId: env.tempoChainId,
      });

      // 3. Store JWT
      setAuthToken(data.authenticate.token);

      // 4. Fetch full user profile
      const meData = await gql<MeResponse>(ME_QUERY);
      setUser(meData.me);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Login failed';
      setError(message);
      throw err;
    } finally {
      setIsLoading(false);
    }
  };

  const logout = () => {
    setAuthToken(null);
    setUser(null);
    setError(null);
    queryClient.clear();
  };

  return {
    user,
    isAuthenticated: () => user() !== null,
    isLoading,
    error,
    register,
    login,
    logout,
  };
}

export type AuthValue = ReturnType<typeof createAuth>;
