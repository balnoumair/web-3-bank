import jwt from "jsonwebtoken";
import type { JwtPayload } from "./context.js";

const JWT_SECRET =
  process.env.JWT_SECRET ?? "dev-secret-change-in-production";
const JWT_EXPIRY = (process.env.JWT_EXPIRY ?? "24h") as jwt.SignOptions["expiresIn"];

export function issueJwt(payload: JwtPayload): string {
  const body: JwtPayload = {
    ...payload,
    chainId:
      typeof payload.chainId === "number" && Number.isFinite(payload.chainId)
        ? payload.chainId
        : Number(process.env.DEFAULT_CHAIN_ID || "1337"),
  };
  return jwt.sign(body, JWT_SECRET, { expiresIn: JWT_EXPIRY });
}

export function verifyJwt(token: string): JwtPayload {
  const p = jwt.verify(token, JWT_SECRET) as JwtPayload & { chainId?: number };
  const fallback = Number(process.env.DEFAULT_CHAIN_ID || "1337");
  return {
    ...p,
    chainId: typeof p.chainId === "number" ? p.chainId : fallback,
  };
}
