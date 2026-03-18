import type { YogaInitialContext } from "graphql-yoga";
import { verifyJwt } from "./jwt.js";

export type JwtPayload = {
  userId: string;
  address: string;
  credentialId: string;
};

export type Context = {
  user: JwtPayload | null;
};

export async function buildContext(
  ctx: YogaInitialContext
): Promise<Context> {
  const authHeader = ctx.request.headers.get("Authorization");
  if (!authHeader?.startsWith("Bearer ")) {
    return { user: null };
  }

  const token = authHeader.slice(7);
  try {
    const user = verifyJwt(token);
    return { user };
  } catch {
    return { user: null };
  }
}
