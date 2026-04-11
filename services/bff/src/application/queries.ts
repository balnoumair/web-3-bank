import type { IUserService, UserRecord } from "../domain/ports/user-service.js";

import type {
  ITreasuryService,
  PoolDepth,
  Transfer,
} from "../domain/ports/treasury-service.js";

export type QueryUseCases = ReturnType<typeof makeQueryUseCases>;

/**
 * Factory that composes query use cases from driven ports.
 * No transport details — only domain orchestration.
 */
export function makeQueryUseCases(
  userService: IUserService,
  treasuryService: ITreasuryService
) {
  return {
    getMe: (address: string): Promise<UserRecord> =>
      userService.getUserByAddress(address),

    getBalance: (address: string): Promise<string> =>
      treasuryService.getBalance(address),

    getPoolDepths: (chainId: number): Promise<PoolDepth> =>
      treasuryService.getPoolDepth(chainId),

    getRecentTransfers: (
      address: string,
      limit: number
    ): Promise<Transfer[]> => treasuryService.getRecentTransfers(address, limit),

    resolveUsername: (username: string): Promise<UserRecord> =>
      userService.getUserByUsername(username),
  };
}
