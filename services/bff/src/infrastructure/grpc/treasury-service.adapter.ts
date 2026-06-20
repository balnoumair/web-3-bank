import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import type {
  BalanceResult,
  ITreasuryService,
  PoolDepth,
  Transfer,
  WithdrawalRoutingEntry,
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

type ActivityEntryProto = {
  kind: string;
  direction: string;
  counterparty: string;
  chainId: string;
  amountWei: string;
  status: string;
  txHash: string;
  occurredAt: string;
};

type WithdrawalRoutingEntryProto = {
  chainId: string;
  withdrawableWei: string;
  available: boolean;
  reason: string;
  balanceWei: string;
};

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

  getBalance(address: string): Promise<BalanceResult> {
    return new Promise((resolve, reject) => {
      this.client.getBalance(
        { address },
        (
          err: grpc.ServiceError | null,
          res: { balanceWei: string; degraded?: boolean },
        ) => {
          if (err) reject(grpcToError(err));
          else
            resolve({
              amountWei: res.balanceWei,
              degraded: res.degraded ?? false,
            });
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

  getAccountActivity(address: string, limit: number): Promise<Transfer[]> {
    return new Promise((resolve, reject) => {
      this.client.getAccountActivity(
        { address, limit },
        (
          err: grpc.ServiceError | null,
          res: { entries: ActivityEntryProto[] },
        ) => {
          if (err) {
            reject(grpcToError(err));
          } else {
            resolve(
              (res.entries ?? []).map((entry) =>
                mapActivityEntry(entry, address),
              ),
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

  getWithdrawalRouting(address: string): Promise<WithdrawalRoutingEntry[]> {
    return new Promise((resolve, reject) => {
      this.client.getWithdrawalRouting(
        { address },
        (
          err: grpc.ServiceError | null,
          res: { entries: WithdrawalRoutingEntryProto[] },
        ) => {
          if (err) reject(grpcToError(err));
          else
            resolve(
              (res.entries ?? []).map((e) => ({
                chainId: String(e.chainId),
                withdrawableWei: e.withdrawableWei,
                available: e.available,
                reason: e.reason ?? "",
                balanceWei: e.balanceWei,
              })),
            );
        },
      );
    });
  }
}

function mapActivityEntry(
  entry: ActivityEntryProto,
  userAddress: string,
): Transfer {
  const user = userAddress.toLowerCase();
  const outgoing = entry.direction === "outgoing";

  let from: string;
  let to: string;

  if (entry.kind === "deposit") {
    from = "";
    to = user;
  } else if (entry.kind === "withdrawal") {
    from = user;
    to = "";
  } else if (outgoing) {
    from = user;
    to = entry.counterparty?.toLowerCase() ?? "";
  } else {
    from = entry.counterparty?.toLowerCase() ?? "";
    to = user;
  }

  const occurredAt = entry.occurredAt?.trim();
  const timestamp =
    occurredAt && /^\d+$/.test(occurredAt)
      ? new Date(Number(occurredAt) * 1000).toISOString()
      : occurredAt || new Date().toISOString();

  return {
    id: entry.txHash,
    from,
    to,
    amount: entry.amountWei,
    timestamp,
    txHash: entry.txHash,
    kind: entry.kind,
    direction: entry.direction,
  };
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
