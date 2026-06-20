export type PoolDepth = {
  chainId: string;
  depthWei: string;
};

export type BalanceResult = {
  amountWei: string;
  degraded: boolean;
};

export type Transfer = {
  id: string;
  from: string;
  to: string;
  amount: string;
  timestamp: string;
  txHash: string;
  kind?: string | null;
  direction?: string | null;
};

export type WithdrawalRoutingEntry = {
  chainId: string;
  withdrawableWei: string;
  available: boolean;
  reason: string;
  balanceWei: string;
};

/** Driven port — implemented by the gRPC treasury-service adapter. */
export interface ITreasuryService {
  getBalance(address: string): Promise<BalanceResult>;
  getPoolDepth(chainId: number): Promise<PoolDepth>;
  getAccountActivity(address: string, limit: number): Promise<Transfer[]>;
  /** RouteReceiver-derived active set (same as hot-path relayer). */
  isChainActive(chainId: number): Promise<boolean>;
  /** Governance-finalized terminal chain state. */
  isChainDecommissioned(chainId: number): Promise<boolean>;
  /** Per-chain withdrawability for withdrawal failover. */
  getWithdrawalRouting(address: string): Promise<WithdrawalRoutingEntry[]>;
}
