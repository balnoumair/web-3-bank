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
    registerUser: async (
      _: unknown,
      args: {
        address: string;
        credentialId: string;
        publicKey: string;
        displayName?: string | null;
        chainId?: number | null;
      }
    ) => {
      return mutations.registerUser(args);
    },

    addCredential: async (
      _: unknown,
      args: { credentialId: string; publicKey: string },
      ctx: Context
    ) => {
      // userId is injected from the JWT — never trusted from the client
      const { userId, address } = requireAuth(ctx);
      return mutations.addCredential({ ...args, userId, address });
    },

    authenticate: async (
      _: unknown,
      args: { credentialId: string; chainId?: number | null }
    ) => {
      return mutations.authenticate(args);
    },

    setUsername: async (
      _: unknown,
      args: { username: string },
      ctx: Context
    ) => {
      const { userId } = requireAuth(ctx);
      return mutations.setUsername({ userId, username: args.username });
    },
  };
}
