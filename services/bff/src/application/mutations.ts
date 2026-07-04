import {
  base64urlToBuffer,
  decodePublicKey,
  deriveTempoAddress,
} from "@repo/tempo-crypto";
import type { IUserService } from "../domain/ports/user-service.js";
import { consumeChallenge, issueChallenge } from "../challenge-store.js";
import { isDevMode } from "../config.js";
import { issueJwt } from "../jwt.js";
import {
  challengeFromClientData,
  verifyAssertion,
  verifyRegistration,
  type WebAuthnAssertionInput,
  type WebAuthnAttestationInput,
} from "../webauthn.js";

export type AuthPayload = { token: string; userId: string };

export type MutationUseCases = ReturnType<typeof makeMutationUseCases>;

function resolvedChainId(chainId?: number | null): number {
  if (typeof chainId === "number" && Number.isFinite(chainId)) {
    return chainId;
  }
  return Number(process.env.DEFAULT_CHAIN_ID || "1337");
}

function credentialIdBuffer(credentialId: string): Buffer {
  return Buffer.from(base64urlToBuffer(credentialId));
}

function authError(message: string): Error {
  return new Error(message);
}

/**
 * Factory that composes mutation use cases from driven ports.
 */
export function makeMutationUseCases(userService: IUserService) {
  return {
    requestChallenge: (): { challenge: string } => ({
      challenge: issueChallenge(),
    }),

    registerUser: async (args: {
      attestation: WebAuthnAttestationInput;
      address: string;
      publicKey: string;
      displayName?: string | null;
      chainId?: number | null;
    }): Promise<AuthPayload> => {
      const challenge = challengeFromClientData(args.attestation.clientDataJSON);
      if (!consumeChallenge(challenge)) {
        throw authError("Invalid or expired registration challenge");
      }

      const registration = await verifyRegistration(args.attestation, challenge);
      if (!registration.verified || !registration.registrationInfo) {
        throw authError("Passkey registration verification failed");
      }

      const spki = decodePublicKey(args.publicKey);
      const derivedAddress = deriveTempoAddress(spki);
      if (derivedAddress.toLowerCase() !== args.address.toLowerCase()) {
        throw authError("Address does not match the passkey public key");
      }

      const storedPublicKey = Buffer.from(
        registration.registrationInfo.credential.publicKey,
      );

      const { userId } = await userService.createUser({
        displayName: args.displayName ?? undefined,
        credentialId: credentialIdBuffer(args.attestation.credentialId),
        publicKey: storedPublicKey,
        tempoAddress: derivedAddress,
      });

      const token = issueJwt({
        userId,
        address: derivedAddress,
        credentialId: args.attestation.credentialId,
        chainId: resolvedChainId(args.chainId),
      });

      return { token, userId };
    },

    authenticate: async (args: {
      assertion: WebAuthnAssertionInput;
      chainId?: number | null;
    }): Promise<AuthPayload> => {
      const challenge = challengeFromClientData(args.assertion.clientDataJSON);
      if (!consumeChallenge(challenge)) {
        throw authError("Invalid or expired authentication challenge");
      }

      const credentialIdBuf = credentialIdBuffer(args.assertion.credentialId);
      const user = await userService.getUserByCredentialId(credentialIdBuf);
      if (user.revoked) {
        throw authError("Credential has been revoked");
      }

      const verification = await verifyAssertion(args.assertion, challenge, {
        id: args.assertion.credentialId,
        publicKey: user.publicKey,
      });
      if (!verification.verified) {
        throw authError("Passkey assertion verification failed");
      }

      const token = issueJwt({
        userId: user.userId,
        address: user.tempoAddress,
        credentialId: args.assertion.credentialId,
        chainId: resolvedChainId(args.chainId),
      });

      return { token, userId: user.userId };
    },

    authenticateLegacy: async (args: {
      credentialId: string;
      chainId?: number | null;
    }): Promise<AuthPayload> => {
      if (!isDevMode()) {
        throw authError("Legacy authenticate is disabled outside dev mode");
      }

      const credentialIdBuf = credentialIdBuffer(args.credentialId);
      const user = await userService.getUserByCredentialId(credentialIdBuf);
      if (user.revoked) {
        throw authError("Credential has been revoked");
      }

      const token = issueJwt({
        userId: user.userId,
        address: user.tempoAddress,
        credentialId: args.credentialId,
        chainId: resolvedChainId(args.chainId),
      });

      return { token, userId: user.userId };
    },

    addCredential: async (args: {
      newCredential: WebAuthnAttestationInput;
      assertion: WebAuthnAssertionInput;
      publicKey: string;
      userId: string;
      address: string;
      sessionCredentialId: string;
    }): Promise<string> => {
      const registerChallenge = challengeFromClientData(
        args.newCredential.clientDataJSON,
      );
      if (!consumeChallenge(registerChallenge)) {
        throw authError("Invalid or expired registration challenge for new credential");
      }

      const loginChallenge = challengeFromClientData(args.assertion.clientDataJSON);
      if (!consumeChallenge(loginChallenge)) {
        throw authError("Invalid or expired assertion challenge");
      }

      const sessionUser = await userService.getUserByCredentialId(
        credentialIdBuffer(args.sessionCredentialId),
      );
      if (sessionUser.revoked || sessionUser.userId !== args.userId) {
        throw authError("Session credential is not valid for this account");
      }

      const assertionCheck = await verifyAssertion(args.assertion, loginChallenge, {
        id: args.assertion.credentialId,
        publicKey: sessionUser.publicKey,
      });
      if (!assertionCheck.verified) {
        throw authError("Existing passkey assertion verification failed");
      }

      const registration = await verifyRegistration(args.newCredential, registerChallenge);
      if (!registration.verified || !registration.registrationInfo) {
        throw authError("New passkey registration verification failed");
      }

      const spki = decodePublicKey(args.publicKey);
      const derivedAddress = deriveTempoAddress(spki);

      const storedPublicKey = Buffer.from(
        registration.registrationInfo.credential.publicKey,
      );

      const { credentialId } = await userService.addCredential({
        userId: args.userId,
        credentialId: credentialIdBuffer(args.newCredential.credentialId),
        publicKey: storedPublicKey,
        tempoAddress: derivedAddress,
      });

      return credentialId;
    },

    setUsername: async (args: {
      userId: string;
      username: string;
    }) => {
      return userService.setUsername(args.userId, args.username);
    },
  };
}
