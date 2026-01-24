import { GraphQLError } from 'graphql';
import type { GraphQLContext } from './context';
import { passkeyService } from '../auth/passkey-service';
import { sessionService } from '../auth/session-service';
import { userStore } from '../storage/user-store';
import type {
    RegistrationResponseJSON,
    AuthenticationResponseJSON,
} from '@simplewebauthn/server/script/deps';

export const resolvers = {
    Query: {
        me: (_parent: unknown, _args: unknown, context: GraphQLContext) => {
            if (!context.user) {
                return null;
            }
            return context.user;
        },

        checkSession: (_parent: unknown, _args: unknown, context: GraphQLContext) => {
            return !!context.user;
        },
    },

    Mutation: {
        startRegistration: async (
            _parent: unknown,
            args: { username: string; displayName: string }
        ) => {
            const { username, displayName } = args;

            // Check if username already exists
            if (userStore.usernameExists(username)) {
                throw new GraphQLError('Username already exists', {
                    extensions: { code: 'USERNAME_EXISTS' },
                });
            }

            // Create user (we need the ID for WebAuthn)
            const user = userStore.createUser(username, displayName);

            // Generate registration options
            const options = await passkeyService.generateRegistrationOptions(
                user.id,
                username,
                displayName
            );

            return { options };
        },

        completeRegistration: async (
            _parent: unknown,
            args: { credential: RegistrationResponseJSON }
        ) => {
            const { credential } = args;

            // Extract challenge from the credential response
            const challenge = credential.response.clientDataJSON;
            const clientData = JSON.parse(
                Buffer.from(challenge, 'base64').toString('utf-8')
            );

            // Verify the registration
            const result = await passkeyService.verifyRegistration(
                credential,
                clientData.challenge
            );

            if (!result.verified || !result.credential) {
                throw new GraphQLError('Registration verification failed', {
                    extensions: { code: 'VERIFICATION_FAILED' },
                });
            }

            // Get the user
            const user = userStore.getUserById(result.credential.userId);
            if (!user) {
                throw new GraphQLError('User not found', {
                    extensions: { code: 'USER_NOT_FOUND' },
                });
            }

            // Create session
            const session = sessionService.createSession(user);

            return session;
        },

        startAuthentication: async (
            _parent: unknown,
            args: { username?: string }
        ) => {
            const { username } = args;

            // If username provided, verify it exists
            if (username && !userStore.usernameExists(username)) {
                throw new GraphQLError('User not found', {
                    extensions: { code: 'USER_NOT_FOUND' },
                });
            }

            // Generate authentication options
            const options = await passkeyService.generateAuthenticationOptions(username);

            return { options };
        },

        completeAuthentication: async (
            _parent: unknown,
            args: { credential: AuthenticationResponseJSON }
        ) => {
            const { credential } = args;

            // Extract challenge from the credential response
            const challenge = credential.response.clientDataJSON;
            const clientData = JSON.parse(
                Buffer.from(challenge, 'base64').toString('utf-8')
            );

            // Verify the authentication
            const result = await passkeyService.verifyAuthentication(
                credential,
                clientData.challenge
            );

            if (!result.verified || !result.userId) {
                throw new GraphQLError('Authentication verification failed', {
                    extensions: { code: 'VERIFICATION_FAILED' },
                });
            }

            // Get the user
            const user = userStore.getUserById(result.userId);
            if (!user) {
                throw new GraphQLError('User not found', {
                    extensions: { code: 'USER_NOT_FOUND' },
                });
            }

            // Create session
            const session = sessionService.createSession(user);

            return session;
        },

        logout: (_parent: unknown, _args: unknown, context: GraphQLContext) => {
            // For JWT-based sessions, logout is handled client-side by removing the token
            // In the future, we could maintain a token blacklist for revocation
            return true;
        },
    },
};
