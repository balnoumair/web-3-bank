export const BALANCE_QUERY = `
  query Balance {
    balance {
      amountWei
      degraded
    }
  }
`;

export interface BalanceResponse {
  balance: {
    amountWei: string;
    degraded?: boolean | null;
  };
}
