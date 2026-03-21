export interface Transfer {
  id: string;
  from: string;
  to: string;
  amount: string;
  timestamp: string;
  txHash: string;
}

export const RECENT_TRANSFERS_QUERY = `
  query RecentTransfers($limit: Int) {
    recentTransfers(limit: $limit) {
      id
      from
      to
      amount
      timestamp
      txHash
    }
  }
`;

export interface RecentTransfersResponse {
  recentTransfers: Transfer[];
}
