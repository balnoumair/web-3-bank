import { startRegistration, startAuthentication } from '@simplewebauthn/browser';
import type {
    PublicKeyCredentialCreationOptionsJSON,
    PublicKeyCredentialRequestOptionsJSON,
} from '@simplewebauthn/browser';
import { graphqlService } from './graphql-client';

export interface User {
    id: string;
    username: string;
    displayName: string;
    createdAt: string;
}

export interface AuthSession {
    token: string;
    user: User;
}

const START_REGISTRATION_MUTATION = `
  mutation StartRegistration($username: String!, $displayName: String!) {
    startRegistration(username: $username, displayName: $displayName) {
      options
    }
  }
`;

const COMPLETE_REGISTRATION_MUTATION = `
  mutation CompleteRegistration($credential: JSON!) {
    completeRegistration(credential: $credential) {
      token
      user {
        id
        username
        displayName
        createdAt
      }
    }
  }
`;

const START_AUTHENTICATION_MUTATION = `
  mutation StartAuthentication($username: String) {
    startAuthentication(username: $username) {
      options
    }
  }
`;

const COMPLETE_AUTHENTICATION_MUTATION = `
  mutation CompleteAuthentication($credential: JSON!) {
    completeAuthentication(credential: $credential) {
      token
      user {
        id
        username
        displayName
        createdAt
      }
    }
  }
`;

export class PasskeyService {
    /**
     * Register a new passkey for a user
     */
    async register(username: string, displayName: string): Promise<AuthSession> {
        try {
            // Step 1: Get registration options from server
            const { startRegistration: { options } } = await graphqlService.request<{
                startRegistration: { options: PublicKeyCredentialCreationOptionsJSON };
            }>(START_REGISTRATION_MUTATION, { username, displayName });

            // Step 2: Create passkey with browser WebAuthn API
            const credential = await startRegistration(options);

            // Step 3: Send credential to server for verification
            const { completeRegistration } = await graphqlService.request<{
                completeRegistration: AuthSession;
            }>(COMPLETE_REGISTRATION_MUTATION, { credential });

            // Step 4: Store auth token
            graphqlService.setAuthToken(completeRegistration.token);

            return completeRegistration;
        } catch (error) {
            console.error('Passkey registration failed:', error);
            throw new Error(
                error instanceof Error ? error.message : 'Failed to register passkey'
            );
        }
    }

    /**
     * Authenticate with an existing passkey
     */
    async authenticate(username?: string): Promise<AuthSession> {
        try {
            // Step 1: Get authentication options from server
            const { startAuthentication: { options } } = await graphqlService.request<{
                startAuthentication: { options: PublicKeyCredentialRequestOptionsJSON };
            }>(START_AUTHENTICATION_MUTATION, { username });

            // Step 2: Get assertion from browser WebAuthn API
            const credential = await startAuthentication(options);

            // Step 3: Send assertion to server for verification
            const { completeAuthentication } = await graphqlService.request<{
                completeAuthentication: AuthSession;
            }>(COMPLETE_AUTHENTICATION_MUTATION, { credential });

            // Step 4: Store auth token
            graphqlService.setAuthToken(completeAuthentication.token);

            return completeAuthentication;
        } catch (error) {
            console.error('Passkey authentication failed:', error);
            throw new Error(
                error instanceof Error ? error.message : 'Failed to authenticate with passkey'
            );
        }
    }

    /**
     * Logout current user
     */
    logout(): void {
        graphqlService.setAuthToken(null);
    }
}

export const passkeyService = new PasskeyService();
