import { createMutation, createQuery, useQueryClient } from '@tanstack/solid-query';
import { gql } from '~/lib/graphql';
import { requestServerChallenge } from '~/lib/auth-challenge';
import {
  createPasskeyCredential,
  getPasskeyCredential,
  webAuthnFieldToBase64url,
} from '~/lib/passkey';
import { deriveTempoAddress, bufferToBase64url } from '~/lib/address';
import {
  CREDENTIALS_QUERY,
  ADD_CREDENTIAL_MUTATION,
  type CredentialsResponse,
  type AddCredentialResponse,
} from '~/queries/auth';

export function useCredentials() {
  return createQuery(() => ({
    queryKey: ['credentials'],
    queryFn: async () => {
      const data = await gql<CredentialsResponse>(CREDENTIALS_QUERY);
      return data.credentials;
    },
  }));
}

export function useAddCredential() {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async (deviceLabel: string) => {
      const registerChallenge = await requestServerChallenge();
      const assertChallenge = await requestServerChallenge();

      const assertion = await getPasskeyCredential(assertChallenge);
      const credential = await createPasskeyCredential(deviceLabel, registerChallenge);

      const data = await gql<AddCredentialResponse>(ADD_CREDENTIAL_MUTATION, {
        newCredential: {
          credentialId: credential.credentialId,
          clientDataJSON: webAuthnFieldToBase64url(credential.clientDataJSON),
          attestationObject: webAuthnFieldToBase64url(credential.attestationObject),
        },
        assertion: {
          credentialId: assertion.credentialId,
          authenticatorData: webAuthnFieldToBase64url(assertion.authenticatorData),
          clientDataJSON: webAuthnFieldToBase64url(assertion.clientDataJSON),
          signature: webAuthnFieldToBase64url(assertion.signature),
        },
        publicKey: bufferToBase64url(credential.publicKey),
      });

      return {
        credentialId: data.addCredential,
        tempoAddress: deriveTempoAddress(credential.publicKey),
      };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['credentials'] });
    },
  }));
}
