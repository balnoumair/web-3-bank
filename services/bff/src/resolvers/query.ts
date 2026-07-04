import { GraphQLError } from "graphql";
import type { Context } from "../context.js";
import type { QueryUseCases } from "../application/queries.js";

function requireAuth(ctx: Context) {
  if (!ctx.user) {
    throw new GraphQLError("Authentication required", {
      extensions: { code: "UNAUTHORIZED" },
    });
  }
  return ctx.user;
}

export function makeQueryResolvers(queries: QueryUseCases) {
  return {
    me: async (_: unknown, __: unknown, ctx: Context) => {
      const { address } = requireAuth(ctx);
      return queries.getMe(address);
    },

    balance: async (_: unknown, __: unknown, ctx: Context) => {
      const { address } = requireAuth(ctx);
      return queries.getBalance(address);
    },

    poolDepths: async (
      _: unknown,
      args: { chainId: number },
      _ctx: Context
    ) => {
      return queries.getPoolDepths(args.chainId);
    },

    recentTransfers: async (
      _: unknown,
      args: { limit?: number | null },
      ctx: Context
    ) => {
      const { address } = requireAuth(ctx);
      return queries.getRecentTransfers(address, args.limit ?? 20);
    },

    resolveUsername: async (
      _: unknown,
      args: { username: string },
      ctx: Context
    ) => {
      const user = requireAuth(ctx);
      return queries.resolveUsername(args.username, user.chainId);
    },

    resolveRecipientRouting: async (
      _: unknown,
      args: { tempoAddress: string },
      ctx: Context
    ) => {
      const user = requireAuth(ctx);
      return queries.resolveRecipientRouting(args.tempoAddress, user.chainId);
    },

    withdrawalRouting: async (_: unknown, __: unknown, ctx: Context) => {
      const { address } = requireAuth(ctx);
      return queries.getWithdrawalRouting(address);
    },

    credentials: async (_: unknown, __: unknown, ctx: Context) => {
      const { userId } = requireAuth(ctx);
      return queries.listCredentials(userId);
    },
  };
}
