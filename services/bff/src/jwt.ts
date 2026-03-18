import jwt from "jsonwebtoken";
import type { JwtPayload } from "./context.js";

const JWT_SECRET =
  process.env.JWT_SECRET ?? "dev-secret-change-in-production";
const JWT_EXPIRY = (process.env.JWT_EXPIRY ?? "24h") as jwt.SignOptions["expiresIn"];

export function issueJwt(payload: JwtPayload): string {
  return jwt.sign(payload, JWT_SECRET, { expiresIn: JWT_EXPIRY });
}

export function verifyJwt(token: string): JwtPayload {
  return jwt.verify(token, JWT_SECRET) as JwtPayload;
}
