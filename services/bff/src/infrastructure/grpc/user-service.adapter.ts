import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import type {
  IUserService,
  UserRecord,
  CreateUserInput,
  AddCredentialInput,
  HomeChainResult,
} from "../../domain/ports/user-service.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

const PROTO_PATH = resolve(
  __dirname,
  "../../../../../packages/proto/user/user_service.proto"
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

/** gRPC adapter for the user-service — implements the IUserService driven port. */
export class GrpcUserServiceAdapter implements IUserService {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private readonly client: any;

  constructor(addr = process.env.USER_SERVICE_ADDR ?? "localhost:50051") {
    this.client = new user.UserService(addr, grpc.credentials.createInsecure());
  }

  createUser(req: CreateUserInput): Promise<{ userId: string }> {
    return new Promise((resolve, reject) => {
      this.client.createUser(
        req,
        (err: grpc.ServiceError | null, res: { userId: string }) => {
          if (err) reject(grpcToError(err));
          else resolve(res);
        }
      );
    });
  }

  getUserByAddress(tempoAddress: string): Promise<UserRecord> {
    return new Promise((resolve, reject) => {
      this.client.getUserByAddress(
        { tempoAddress },
        (err: grpc.ServiceError | null, res: UserRecord) => {
          if (err) reject(grpcToError(err));
          else resolve(res);
        }
      );
    });
  }

  getUserByCredentialId(credentialId: Buffer): Promise<UserRecord> {
    return new Promise((resolve, reject) => {
      this.client.getUserByCredentialId(
        { credentialId },
        (err: grpc.ServiceError | null, res: UserRecord) => {
          if (err) reject(grpcToError(err));
          else resolve(res);
        }
      );
    });
  }

  addCredential(req: AddCredentialInput): Promise<{ credentialId: string }> {
    return new Promise((resolve, reject) => {
      this.client.addCredential(
        req,
        (err: grpc.ServiceError | null, res: { credentialId: string }) => {
          if (err) reject(grpcToError(err));
          else resolve(res);
        }
      );
    });
  }

  setUsername(userId: string, username: string): Promise<UserRecord> {
    return new Promise((resolve, reject) => {
      this.client.setUsername(
        { userId, username },
        (err: grpc.ServiceError | null, res: UserRecord) => {
          if (err) reject(grpcToError(err));
          else resolve(res);
        }
      );
    });
  }

  getUserByUsername(username: string): Promise<UserRecord> {
    return new Promise((resolve, reject) => {
      this.client.getUserByUsername(
        { username },
        (err: grpc.ServiceError | null, res: UserRecord) => {
          if (err) reject(grpcToError(err));
          else resolve(res);
        }
      );
    });
  }

  getUserHomeChain(tempoAddress: string): Promise<HomeChainResult> {
    return new Promise((resolve, reject) => {
      this.client.getUserHomeChain(
        { tempoAddress },
        (
          err: grpc.ServiceError | null,
          res: { found: boolean; chainId: string | number }
        ) => {
          if (err) reject(grpcToError(err));
          else if (res.found) {
            resolve({ found: true, chainId: String(res.chainId) });
          } else {
            resolve({ found: false });
          }
        }
      );
    });
  }
}

function grpcToError(err: grpc.ServiceError): Error {
  // Use err.details for the server-set message (e.g. "username already taken").
  // Fall back to err.message for unknown errors.
  const message =
    err.code === grpc.status.NOT_FOUND
      ? "Not found"
      : err.code === grpc.status.UNAUTHENTICATED
      ? "Unauthenticated"
      : err.details || err.message;
  return new Error(message);
}
