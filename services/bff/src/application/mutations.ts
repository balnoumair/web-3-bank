import type { IUserService, UserRecord } from "../domain/ports/user-service.js";
import { issueJwt } from "../jwt.js";

export type AuthPayload = { token: string; userId: string };

export type MutationUseCases = ReturnType<typeof makeMutationUseCases>;

function resolvedChainId(chainId?: number | null): number {
  if (typeof chainId === "number" && Number.isFinite(chainId)) {
    return chainId;
  }
  return Number(process.env.DEFAULT_CHAIN_ID || "1337");
}

/** Decode a base64url string (from the browser WebAuthn API) into a Buffer. */
function base64urlToBuffer(b64url: string): Buffer {
  const base64 = b64url.replace(/-/g, "+").replace(/_/g, "/");
  return Buffer.from(base64, "base64");
}

/**
 * Factory that composes mutation use cases from driven ports.
 * Owns the WebAuthn → Buffer conversion and JWT issuance logic.
 */
export function makeMutationUseCases(userService: IUserService) {
  return {
    registerUser: async (args: {
      address: string;
      credentialId: string;
      publicKey: string;
      displayName?: string | null;
      chainId?: number | null;
    }): Promise<AuthPayload> => {
      const { userId } = await userService.createUser({
        displayName: args.displayName ?? undefined,
        credentialId: base64urlToBuffer(args.credentialId),
        publicKey: base64urlToBuffer(args.publicKey),
        tempoAddress: args.address,
      });

      const token = issueJwt({
        userId,
        address: args.address,
        credentialId: args.credentialId,
        chainId: resolvedChainId(args.chainId),
      });

      return { token, userId };
    },

    authenticate: async (args: {
      credentialId: string;
      chainId?: number | null;
    }): Promise<AuthPayload> => {
      // Look up the user by their WebAuthn credential ID (the passkey
      // assertion doesn't return the public key, so we can't derive the
      // address client-side during login).
      const credentialIdBuf = base64urlToBuffer(args.credentialId);
      const { userId, tempoAddress } =
        await userService.getUserByCredentialId(credentialIdBuf);

      const token = issueJwt({
        userId,
        address: tempoAddress,
        credentialId: args.credentialId,
        chainId: resolvedChainId(args.chainId),
      });

      return { token, userId };
    },

    addCredential: async (args: {
      credentialId: string;
      publicKey: string;
      userId: string;
      address: string;
    }): Promise<string> => {
      const { credentialId } = await userService.addCredential({
        userId: args.userId,
        credentialId: base64urlToBuffer(args.credentialId),
        publicKey: base64urlToBuffer(args.publicKey),
        tempoAddress: args.address,
      });

      return credentialId;
    },

    setUsername: async (args: {
      userId: string;
      username: string;
    }): Promise<UserRecord> => {
      return userService.setUsername(args.userId, args.username);
    },
  };
}
