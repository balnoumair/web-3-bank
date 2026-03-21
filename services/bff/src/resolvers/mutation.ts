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
      args: { address: string; credentialId: string }
    ) => {
      return mutations.authenticate(args);
    },
  };
}
