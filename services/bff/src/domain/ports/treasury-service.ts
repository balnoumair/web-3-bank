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

/** Driven port — implemented by the HTTP treasury-service adapter. */
export interface ITreasuryService {
  getBalance(address: string): Promise<string>;
  getPoolDepth(chainId: number): Promise<PoolDepth>;
  getRecentTransfers(address: string, limit: number): Promise<Transfer[]>;
}
