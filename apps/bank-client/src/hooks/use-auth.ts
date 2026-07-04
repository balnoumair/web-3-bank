import { createSignal, onMount } from 'solid-js';
import {
  createPasskeyCredential,
  getPasskeyCredential,
  webAuthnFieldToBase64url,
} from '~/lib/passkey';
import { deriveTempoAddress, bufferToBase64url } from '~/lib/address';
import { gql, setAuthToken, getAuthToken } from '~/lib/graphql';
import { queryClient } from '~/contexts/query-context';
import { requestServerChallenge } from '~/lib/auth-challenge';
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
      const challenge = await requestServerChallenge();
      const credential = await createPasskeyCredential(displayName, challenge);
      const address = deriveTempoAddress(credential.publicKey);

      const data = await gql<RegisterUserResponse>(REGISTER_USER_MUTATION, {
        attestation: {
          credentialId: credential.credentialId,
          clientDataJSON: webAuthnFieldToBase64url(credential.clientDataJSON),
          attestationObject: webAuthnFieldToBase64url(credential.attestationObject),
        },
        address,
        publicKey: bufferToBase64url(credential.publicKey),
        displayName,
        chainId: env.tempoChainId,
      });

      setAuthToken(data.registerUser.token);

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
      const challenge = await requestServerChallenge();
      const assertion = await getPasskeyCredential(challenge);

      const data = await gql<AuthenticateResponse>(AUTHENTICATE_MUTATION, {
        assertion: {
          credentialId: assertion.credentialId,
          authenticatorData: webAuthnFieldToBase64url(assertion.authenticatorData),
          clientDataJSON: webAuthnFieldToBase64url(assertion.clientDataJSON),
          signature: webAuthnFieldToBase64url(assertion.signature),
        },
        chainId: env.tempoChainId,
      });

      setAuthToken(data.authenticate.token);

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
