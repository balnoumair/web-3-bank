export const BALANCE_QUERY = `
  query Balance {
    balance
  }
`;

export interface BalanceResponse {
  balance: string;
}
