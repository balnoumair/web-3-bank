import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import type {
  ITreasuryService,
  PoolDepth,
  Transfer,
} from "../../domain/ports/treasury-service.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

const PROTO_PATH = resolve(
  __dirname,
  "../../../../../packages/proto/treasury/treasury_service.proto",
);

const packageDef = protoLoader.loadSync(PROTO_PATH, {
  keepCase: false,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { treasury } = grpc.loadPackageDefinition(packageDef) as any;

/** gRPC adapter for the treasury-service — implements the ITreasuryService driven port. */
export class GrpcTreasuryServiceAdapter implements ITreasuryService {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private readonly client: any;

  constructor(addr = process.env.TREASURY_SERVICE_ADDR ?? "localhost:50052") {
    this.client = new treasury.TreasuryService(
      addr,
      grpc.credentials.createInsecure(),
    );
  }

  getBalance(address: string): Promise<string> {
    return new Promise((resolve, reject) => {
      this.client.getBalance(
        { address },
        (err: grpc.ServiceError | null, res: { balanceWei: string }) => {
          if (err) reject(grpcToError(err));
          else resolve(res.balanceWei);
        },
      );
    });
  }

  getPoolDepth(chainId: number): Promise<PoolDepth> {
    return new Promise((resolve, reject) => {
      this.client.getPoolDepth(
        { chainId: chainId.toString() },
        (err: grpc.ServiceError | null, res: { depthWei: string }) => {
          if (err) reject(grpcToError(err));
          else resolve({ chainId: chainId.toString(), depthWei: res.depthWei });
        },
      );
    });
  }

  getRecentTransfers(address: string, limit: number): Promise<Transfer[]> {
    return new Promise((resolve, reject) => {
      this.client.getRecentTransfers(
        { address, limit },
        (
          err: grpc.ServiceError | null,
          res: {
            transfers: Array<{
              sourceEventHash: string;
              sourceChainId: string;
              destChainId: string;
              sender: string;
              recipient: string;
              amountWei: string;
              status: string;
              createdAt: string;
            }>;
          },
        ) => {
          if (err) {
            reject(grpcToError(err));
          } else {
            resolve(
              res.transfers.map((t) => ({
                id: t.sourceEventHash,
                from: t.sender,
                to: t.recipient,
                amount: t.amountWei,
                timestamp: t.createdAt,
                txHash: t.sourceEventHash,
              })),
            );
          }
        },
      );
    });
  }

  isChainActive(chainId: number): Promise<boolean> {
    return new Promise((resolve, reject) => {
      this.client.isChainActive(
        { chainId: chainId.toString() },
        (err: grpc.ServiceError | null, res: { active: boolean }) => {
          if (err) reject(grpcToError(err));
          else resolve(res.active);
        },
      );
    });
  }

  isChainDecommissioned(chainId: number): Promise<boolean> {
    return new Promise((resolve, reject) => {
      this.client.isChainDecommissioned(
        { chainId: chainId.toString() },
        (err: grpc.ServiceError | null, res: { decommissioned: boolean }) => {
          if (err) reject(grpcToError(err));
          else resolve(res.decommissioned);
        },
      );
    });
  }
}

function grpcToError(err: grpc.ServiceError): Error {
  const message =
    err.code === grpc.status.NOT_FOUND
      ? "Not found"
      : err.code === grpc.status.UNIMPLEMENTED
        ? "Not implemented"
        : err.message;
  return new Error(message);
}
