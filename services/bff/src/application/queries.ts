import type { IUserService, UserRecord } from "../domain/ports/user-service.js";
import { resolveDestChainId } from "../domain/send-routing.js";

import type {
  BalanceResult,
  ITreasuryService,
  PoolDepth,
  Transfer,
  WithdrawalRoutingEntry,
} from "../domain/ports/treasury-service.js";

export type QueryUseCases = ReturnType<typeof makeQueryUseCases>;

async function resolveHotPathDestChainId(
  userService: IUserService,
  treasuryService: ITreasuryService,
  recipientTempoAddress: string,
  senderChainId: number,
): Promise<string> {
  let home: bigint | undefined;
  try {
    const hc = await userService.getUserHomeChain(recipientTempoAddress);
    if (hc.found) {
      home = BigInt(hc.chainId);
    }
  } catch (e) {
    console.warn("user-service unavailable for home-chain routing:", e);
    return String(senderChainId);
  }

  let homeActive = true;
  let homeDecommissioned = false;
  if (home !== undefined) {
    try {
      homeActive = await treasuryService.isChainActive(Number(home));
      homeDecommissioned = await treasuryService.isChainDecommissioned(
        Number(home),
      );
    } catch (e) {
      console.warn(
        "treasury IsChainActive failed — falling back to same-chain:",
        e,
      );
      homeActive = false;
      homeDecommissioned = false;
    }
  }

  const dest = resolveDestChainId({
    senderChainId: BigInt(senderChainId),
    recipientHomeChainId: home,
    recipientHomeChainActive: homeActive,
    recipientHomeChainDecommissioned: homeDecommissioned,
  });
  return dest.toString();
}

/**
 * Factory that composes query use cases from driven ports.
 * No transport details — only domain orchestration.
 */
export function makeQueryUseCases(
  userService: IUserService,
  treasuryService: ITreasuryService,
) {
  return {
    getMe: async (address: string): Promise<UserRecord> => {
      const u = await userService.getUserByAddress(address);
      return { ...u, destChainId: null };
    },

    getBalance: (address: string): Promise<BalanceResult> =>
      treasuryService.getBalance(address),

    getPoolDepths: (chainId: number): Promise<PoolDepth> =>
      treasuryService.getPoolDepth(chainId),

    getRecentTransfers: (address: string, limit: number): Promise<Transfer[]> =>
      treasuryService.getAccountActivity(address, limit),

    getWithdrawalRouting: (address: string): Promise<WithdrawalRoutingEntry[]> =>
      treasuryService.getWithdrawalRouting(address),

    listCredentials: (userId: string) => userService.listCredentials(userId),

    resolveUsername: async (
      username: string,
      senderChainId: number,
    ): Promise<UserRecord> => {
      const u = await userService.getUserByUsername(username);
      const destChainId = await resolveHotPathDestChainId(
        userService,
        treasuryService,
        u.tempoAddress,
        senderChainId,
      );
      return { ...u, destChainId };
    },

    resolveRecipientRouting: async (
      tempoAddress: string,
      senderChainId: number,
    ): Promise<{ tempoAddress: string; destChainId: string }> => {
      const trimmed = tempoAddress.trim();
      if (!/^0x[0-9a-fA-F]{40}$/.test(trimmed)) {
        throw new Error("Invalid Tempo address");
      }
      const normalized = trimmed.toLowerCase();
      const destChainId = await resolveHotPathDestChainId(
        userService,
        treasuryService,
        normalized,
        senderChainId,
      );
      return { tempoAddress: normalized, destChainId };
    },
  };
}
