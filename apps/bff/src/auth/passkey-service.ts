import {
    generateRegistrationOptions,
    verifyRegistrationResponse,
    generateAuthenticationOptions,
    verifyAuthenticationResponse,
    type VerifiedRegistrationResponse,
    type VerifiedAuthenticationResponse,
} from '@simplewebauthn/server';
import type {
    PublicKeyCredentialCreationOptionsJSON,
    PublicKeyCredentialRequestOptionsJSON,
    RegistrationResponseJSON,
    AuthenticationResponseJSON,
} from '@simplewebauthn/server/script/deps';
import { userStore } from '../storage/user-store';
import type { PasskeyCredential } from '../types';

const RP_NAME = process.env.RP_NAME || 'Web3Bank';
const RP_ID = process.env.RP_ID || 'localhost';
const RP_ORIGIN = process.env.RP_ORIGIN || 'http://localhost:3000';

export class PasskeyService {
    /**
     * Generate registration options for a new passkey
     */
    async generateRegistrationOptions(
        userId: string,
        username: string,
        displayName: string
    ): Promise<PublicKeyCredentialCreationOptionsJSON> {
        // Get existing credentials for this user to exclude them
        const existingCredentials = userStore.getCredentialsByUserId(userId);

        const options = await generateRegistrationOptions({
            rpName: RP_NAME,
            rpID: RP_ID,
            userID: userId,
            userName: username,
            userDisplayName: displayName,
            attestationType: 'none',
            excludeCredentials: existingCredentials.map((cred) => ({
                id: cred.credentialId,
                type: 'public-key',
                transports: cred.transports,
            })),
            authenticatorSelection: {
                residentKey: 'preferred',
                userVerification: 'preferred',
                authenticatorAttachment: 'platform',
            },
        });

        // Store challenge for verification
        userStore.storeChallenge(options.challenge, userId);

        return options;
    }

    /**
     * Verify registration response and store credential
     */
    async verifyRegistration(
        response: RegistrationResponseJSON,
        expectedChallenge: string
    ): Promise<{ verified: boolean; credential?: PasskeyCredential }> {
        let verification: VerifiedRegistrationResponse;

        try {
            verification = await verifyRegistrationResponse({
                response,
                expectedChallenge,
                expectedOrigin: RP_ORIGIN,
                expectedRPID: RP_ID,
            });
        } catch (error) {
            console.error('Registration verification failed:', error);
            return { verified: false };
        }

        const { verified, registrationInfo } = verification;

        if (!verified || !registrationInfo) {
            return { verified: false };
        }

        // Get the challenge to find the associated user
        const challengeData = userStore.getChallenge(expectedChallenge);
        if (!challengeData || !challengeData.userId) {
            return { verified: false };
        }

        // Create credential object
        const credential: PasskeyCredential = {
            credentialId: registrationInfo.credentialID,
            publicKey: registrationInfo.credentialPublicKey,
            counter: registrationInfo.counter,
            userId: challengeData.userId,
            createdAt: new Date(),
            transports: response.response.transports,
        };

        // Store credential
        userStore.storeCredential(credential);

        // Delete used challenge
        userStore.deleteChallenge(expectedChallenge);

        return { verified: true, credential };
    }

    /**
     * Generate authentication options for login
     */
    async generateAuthenticationOptions(
        username?: string
    ): Promise<PublicKeyCredentialRequestOptionsJSON> {
        let allowCredentials: { id: string; type: 'public-key'; transports?: AuthenticatorTransport[] }[] = [];

        // If username provided, only allow credentials for that user
        if (username) {
            const user = userStore.getUserByUsername(username);
            if (user) {
                const credentials = userStore.getCredentialsByUserId(user.id);
                allowCredentials = credentials.map((cred) => ({
                    id: cred.credentialId,
                    type: 'public-key' as const,
                    transports: cred.transports,
                }));
            }
        }

        const options = await generateAuthenticationOptions({
            rpID: RP_ID,
            allowCredentials: allowCredentials.length > 0 ? allowCredentials : undefined,
            userVerification: 'preferred',
        });

        // Store challenge for verification
        userStore.storeChallenge(options.challenge);

        return options;
    }

    /**
     * Verify authentication response
     */
    async verifyAuthentication(
        response: AuthenticationResponseJSON,
        expectedChallenge: string
    ): Promise<{ verified: boolean; userId?: string }> {
        // Get the credential
        const credential = userStore.getCredential(response.id);
        if (!credential) {
            console.error('Credential not found:', response.id);
            return { verified: false };
        }

        let verification: VerifiedAuthenticationResponse;

        try {
            verification = await verifyAuthenticationResponse({
                response,
                expectedChallenge,
                expectedOrigin: RP_ORIGIN,
                expectedRPID: RP_ID,
                authenticator: {
                    credentialID: credential.credentialId,
                    credentialPublicKey: credential.publicKey,
                    counter: credential.counter,
                },
            });
        } catch (error) {
            console.error('Authentication verification failed:', error);
            return { verified: false };
        }

        const { verified, authenticationInfo } = verification;

        if (!verified) {
            return { verified: false };
        }

        // Update counter
        userStore.updateCredentialCounter(
            credential.credentialId,
            authenticationInfo.newCounter
        );

        // Delete used challenge
        userStore.deleteChallenge(expectedChallenge);

        return { verified: true, userId: credential.userId };
    }
}

export const passkeyService = new PasskeyService();
