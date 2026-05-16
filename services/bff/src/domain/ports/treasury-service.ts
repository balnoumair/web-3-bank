export type PoolDepth = {
  chainId: string;
  depthWei: string;
};

export type Transfer = {
  id: string;
  from: string;
  to: string;
  amount: string;
  timestamp: string;
  txHash: string;
};

/** Driven port — implemented by the gRPC treasury-service adapter. */
export interface ITreasuryService {
  getBalance(address: string): Promise<string>;
  getPoolDepth(chainId: number): Promise<PoolDepth>;
  getRecentTransfers(address: string, limit: number): Promise<Transfer[]>;
  /** RouteReceiver-derived active set (same as hot-path relayer). */
  isChainActive(chainId: number): Promise<boolean>;
  /** Governance-finalized terminal chain state. */
  isChainDecommissioned(chainId: number): Promise<boolean>;
}
