import { GraphQLError } from "graphql";
import type { Context } from "../context.js";
import {
  createUser,
  addCredential,
  getUserByAddress,
  base64urlToBuffer,
} from "../grpc/user-client.js";
import { issueJwt } from "../jwt.js";

function requireAuth(ctx: Context) {
  if (!ctx.user) {
    throw new GraphQLError("Authentication required", {
      extensions: { code: "UNAUTHORIZED" },
    });
  }
  return ctx.user;
}

export const mutationResolvers = {
  registerUser: async (
    _: unknown,
    args: {
      address: string;
      credentialId: string;
      publicKey: string;
      displayName?: string | null;
    }
  ) => {
    const credentialIdBytes = base64urlToBuffer(args.credentialId);
    const publicKeyBytes = base64urlToBuffer(args.publicKey);

    const { userId } = await createUser({
      displayName: args.displayName ?? undefined,
      credentialId: credentialIdBytes,
      publicKey: publicKeyBytes,
      tempoAddress: args.address,
    });

    const token = issueJwt({
      userId,
      address: args.address,
      credentialId: args.credentialId,
    });

    return { token, userId };
  },

  addCredential: async (
    _: unknown,
    args: { credentialId: string; publicKey: string },
    ctx: Context
  ) => {
    // user_id is injected from JWT — never trusted from the client
    const { userId, address } = requireAuth(ctx);

    const credentialIdBytes = base64urlToBuffer(args.credentialId);
    const publicKeyBytes = base64urlToBuffer(args.publicKey);

    const { credentialId } = await addCredential({
      userId,
      credentialId: credentialIdBytes,
      publicKey: publicKeyBytes,
      tempoAddress: address,
    });

    return credentialId;
  },

  authenticate: async (
    _: unknown,
    args: { address: string; credentialId: string }
  ) => {
    // The frontend has already verified the WebAuthn passkey challenge before
    // calling this mutation. The BFF confirms the user exists, then issues a JWT.
    const userRecord = await getUserByAddress(args.address);

    const token = issueJwt({
      userId: userRecord.userId,
      address: args.address,
      credentialId: args.credentialId,
    });

    return { token, userId: userRecord.userId };
  },
};
