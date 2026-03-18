const TREASURY_SERVICE_URL =
  process.env.TREASURY_SERVICE_URL ?? "http://localhost:8080";

async function fetchJson<T>(path: string): Promise<T> {
  const res = await fetch(`${TREASURY_SERVICE_URL}${path}`);
  if (!res.ok) {
    throw new Error(
      `Treasury request failed: ${res.status} ${res.statusText} (${path})`
    );
  }
  return res.json() as Promise<T>;
}

// ── Queries ───────────────────────────────────────────────────────────────────

/**
 * Returns the SyncUSD balance (as a decimal string) for the given address.
 * Treasury endpoint: GET /api/v1/balance/:address
 */
export async function getBalance(address: string): Promise<string> {
  const data = await fetchJson<{ balance: string }>(
    `/api/v1/balance/${encodeURIComponent(address)}`
  );
  return data.balance;
}

export type PoolDepth = {
  chainId: string;
  depthWei: string;
};

/**
 * Returns the pool depth in wei for a given chain ID.
 * Treasury endpoint: GET /api/v1/pool-depths/:chainId
 */
export async function getPoolDepth(chainId: number): Promise<PoolDepth> {
  const data = await fetchJson<PoolDepth>(`/api/v1/pool-depths/${chainId}`);
  return data;
}

export type Transfer = {
  id: string;
  from: string;
  to: string;
  amount: string;
  timestamp: string;
  txHash: string;
};

/**
 * Returns recent transfers for the given address.
 * Treasury endpoint: GET /api/v1/transfers/:address?limit=N
 */
export async function getRecentTransfers(
  address: string,
  limit: number
): Promise<Transfer[]> {
  const data = await fetchJson<{ transfers: Transfer[] }>(
    `/api/v1/transfers/${encodeURIComponent(address)}?limit=${limit}`
  );
  return data.transfers;
}
