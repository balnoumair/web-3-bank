import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Resolve proto path relative to this file: services/bff/src/grpc/ → root → packages/proto/
const PROTO_PATH = resolve(
  __dirname,
  "../../../../packages/proto/user/user_service.proto"
);

const packageDef = protoLoader.loadSync(PROTO_PATH, {
  keepCase: false,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { user } = grpc.loadPackageDefinition(packageDef) as any;

const USER_SERVICE_ADDR =
  process.env.USER_SERVICE_ADDR ?? "localhost:50051";

// Single shared client instance
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const client = new user.UserService(
  USER_SERVICE_ADDR,
  grpc.credentials.createInsecure()
);

// ── Promisified wrappers ──────────────────────────────────────────────────────

export type UserRecord = {
  userId: string;
  displayName: string;
  status: string;
  tempoAddress: string;
};

export function createUser(req: {
  displayName?: string;
  credentialId: Buffer;
  publicKey: Buffer;
  tempoAddress: string;
}): Promise<{ userId: string }> {
  return new Promise((resolve, reject) => {
    client.createUser(
      req,
      (err: grpc.ServiceError | null, res: { userId: string }) => {
        if (err) reject(grpcToError(err));
        else resolve(res);
      }
    );
  });
}

export function getUserByAddress(tempoAddress: string): Promise<UserRecord> {
  return new Promise((resolve, reject) => {
    client.getUserByAddress(
      { tempoAddress },
      (err: grpc.ServiceError | null, res: UserRecord) => {
        if (err) reject(grpcToError(err));
        else resolve(res);
      }
    );
  });
}

export function addCredential(req: {
  userId: string;
  credentialId: Buffer;
  publicKey: Buffer;
  tempoAddress: string;
}): Promise<{ credentialId: string }> {
  return new Promise((resolve, reject) => {
    client.addCredential(
      req,
      (err: grpc.ServiceError | null, res: { credentialId: string }) => {
        if (err) reject(grpcToError(err));
        else resolve(res);
      }
    );
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Decode a base64url string (from browser WebAuthn API) into a Buffer. */
export function base64urlToBuffer(b64url: string): Buffer {
  const base64 = b64url.replace(/-/g, "+").replace(/_/g, "/");
  return Buffer.from(base64, "base64");
}

/** Map gRPC status codes to descriptive Error messages. */
function grpcToError(err: grpc.ServiceError): Error {
  const message =
    err.code === grpc.status.NOT_FOUND
      ? "Not found"
      : err.code === grpc.status.ALREADY_EXISTS
      ? "Already exists"
      : err.code === grpc.status.UNAUTHENTICATED
      ? "Unauthenticated"
      : err.message;
  return new Error(message);
}
