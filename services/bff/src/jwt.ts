import jwt from "jsonwebtoken";
import type { JwtPayload } from "./context.js";
import { isDevMode } from "./config.js";

const JWT_EXPIRY = (process.env.JWT_EXPIRY ?? "24h") as jwt.SignOptions["expiresIn"];

function jwtSecret(): string {
  const secret = process.env.JWT_SECRET;
  if (secret) {
    return secret;
  }
  if (isDevMode()) {
    return "dev-secret-change-in-production";
  }
  throw new Error("JWT_SECRET is not configured");
}

export function issueJwt(payload: JwtPayload): string {
  const body: JwtPayload = {
    ...payload,
    chainId:
      typeof payload.chainId === "number" && Number.isFinite(payload.chainId)
        ? payload.chainId
        : Number(process.env.DEFAULT_CHAIN_ID || "1337"),
  };
  return jwt.sign(body, jwtSecret(), { expiresIn: JWT_EXPIRY });
}

export function verifyJwt(token: string): JwtPayload {
  const p = jwt.verify(token, jwtSecret()) as JwtPayload & { chainId?: number };
  const fallback = Number(process.env.DEFAULT_CHAIN_ID || "1337");
  return {
    ...p,
    chainId: typeof p.chainId === "number" ? p.chainId : fallback,
  };
}
