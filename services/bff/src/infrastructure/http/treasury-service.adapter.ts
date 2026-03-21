import type {
  ITreasuryService,
  PoolDepth,
  Transfer,
} from "../../domain/ports/treasury-service.js";

/** HTTP adapter for the treasury-service — implements the ITreasuryService driven port. */
export class HttpTreasuryServiceAdapter implements ITreasuryService {
  private readonly baseUrl: string;

  constructor(
    baseUrl = process.env.TREASURY_SERVICE_URL ?? "http://localhost:8080"
  ) {
    this.baseUrl = baseUrl;
  }

  async getBalance(address: string): Promise<string> {
    const data = await this.fetchJson<{ balance: string }>(
      `/api/v1/balance/${encodeURIComponent(address)}`
    );
    return data.balance;
  }

  async getPoolDepth(chainId: number): Promise<PoolDepth> {
    return this.fetchJson<PoolDepth>(`/api/v1/pool-depths/${chainId}`);
  }

  async getRecentTransfers(address: string, limit: number): Promise<Transfer[]> {
    const data = await this.fetchJson<{ transfers: Transfer[] }>(
      `/api/v1/transfers/${encodeURIComponent(address)}?limit=${limit}`
    );
    return data.transfers;
  }

  private async fetchJson<T>(path: string): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`);
    if (!res.ok) {
      throw new Error(
        `Treasury request failed: ${res.status} ${res.statusText} (${path})`
      );
    }
    return res.json() as Promise<T>;
  }
}
