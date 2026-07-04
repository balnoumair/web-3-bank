import { GraphQLError } from "graphql";
import type { Context } from "../context.js";
import type { MutationUseCases } from "../application/mutations.js";

function requireAuth(ctx: Context) {
  if (!ctx.user) {
    throw new GraphQLError("Authentication required", {
      extensions: { code: "UNAUTHORIZED" },
    });
  }
  return ctx.user;
}

export function makeMutationResolvers(mutations: MutationUseCases) {
  return {
    requestChallenge: () => mutations.requestChallenge(),

    registerUser: async (
      _: unknown,
      args: {
        attestation: {
          credentialId: string;
          clientDataJSON: string;
          attestationObject: string;
        };
        address: string;
        publicKey: string;
        displayName?: string | null;
        chainId?: number | null;
      },
    ) => {
      try {
        return await mutations.registerUser(args);
      } catch (err) {
        throw authGraphqlError(err);
      }
    },

    addCredential: async (
      _: unknown,
      args: {
        newCredential: {
          credentialId: string;
          clientDataJSON: string;
          attestationObject: string;
        };
        assertion: {
          credentialId: string;
          authenticatorData: string;
          clientDataJSON: string;
          signature: string;
        };
        publicKey: string;
      },
      ctx: Context,
    ) => {
      const { userId, address, credentialId } = requireAuth(ctx);
      try {
        return await mutations.addCredential({
          ...args,
          userId,
          address,
          sessionCredentialId: credentialId,
        });
      } catch (err) {
        throw authGraphqlError(err);
      }
    },

    authenticate: async (
      _: unknown,
      args: {
        assertion: {
          credentialId: string;
          authenticatorData: string;
          clientDataJSON: string;
          signature: string;
        };
        chainId?: number | null;
      },
    ) => {
      try {
        return await mutations.authenticate(args);
      } catch (err) {
        throw authGraphqlError(err);
      }
    },

    authenticateLegacy: async (
      _: unknown,
      args: { credentialId: string; chainId?: number | null },
    ) => {
      try {
        return await mutations.authenticateLegacy(args);
      } catch (err) {
        throw authGraphqlError(err);
      }
    },

    setUsername: async (
      _: unknown,
      args: { username: string },
      ctx: Context,
    ) => {
      const { userId } = requireAuth(ctx);
      return mutations.setUsername({ userId, username: args.username });
    },
  };
}

function authGraphqlError(err: unknown): GraphQLError {
  const message = err instanceof Error ? err.message : "Authentication failed";
  return new GraphQLError(message, {
    extensions: { code: "UNAUTHENTICATED" },
  });
}
