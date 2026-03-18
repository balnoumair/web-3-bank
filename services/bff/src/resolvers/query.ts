import { GraphQLError } from "graphql";
import type { Context } from "../context.js";
import { getUserByAddress } from "../grpc/user-client.js";
import {
  getBalance,
  getPoolDepth,
  getRecentTransfers,
} from "../http/treasury-client.js";

function requireAuth(ctx: Context) {
  if (!ctx.user) {
    throw new GraphQLError("Authentication required", {
      extensions: { code: "UNAUTHORIZED" },
    });
  }
  return ctx.user;
}

export const queryResolvers = {
  me: async (_: unknown, __: unknown, ctx: Context) => {
    const { address } = requireAuth(ctx);
    return getUserByAddress(address);
  },

  balance: async (_: unknown, __: unknown, ctx: Context) => {
    const { address } = requireAuth(ctx);
    return getBalance(address);
  },

  poolDepths: async (
    _: unknown,
    args: { chainId: number },
    _ctx: Context
  ) => {
    return getPoolDepth(args.chainId);
  },

  recentTransfers: async (
    _: unknown,
    args: { limit?: number | null },
    ctx: Context
  ) => {
    const { address } = requireAuth(ctx);
    return getRecentTransfers(address, args.limit ?? 20);
  },
};
