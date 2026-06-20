import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { encodeFunctionData } from 'viem';
import { bankAbi, BANK_ADDRESS, USDC_ADDRESS } from '~/config/contracts';
import { env } from '~/config/env';
import { gql } from '~/lib/graphql';
import { selectWithdrawalChain } from '~/lib/withdrawal-routing';
import { signAndSendPasskeyTx, waitForTx } from '~/lib/viem-client';
import {
  WITHDRAWAL_ROUTING_QUERY,
  type WithdrawalRoutingResponse,
} from '~/queries/auth';

export function useWithdraw(userAddress: () => `0x${string}` | undefined) {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async ({ amount }: { amount: bigint }) => {
      const from = userAddress();
      if (!from) throw new Error('Not connected');

      const routing = await gql<WithdrawalRoutingResponse>(WITHDRAWAL_ROUTING_QUERY);
      const selection = selectWithdrawalChain(
        routing.withdrawalRouting,
        amount,
        String(env.tempoChainId),
      );

      if (!selection.ok) {
        throw new Error(selection.reason);
      }

      if (selection.chainId !== String(env.tempoChainId)) {
        throw new Error(
          'Your withdrawable balance is on another chain. Multi-chain withdrawal from this app is not supported yet — funds remain safe until the chain recovers or is decommissioned.',
        );
      }

      const data = encodeFunctionData({
        abi: bankAbi,
        functionName: 'withdraw',
        args: [USDC_ADDRESS, amount],
      });

      const txHash = await signAndSendPasskeyTx({
        from,
        to: BANK_ADDRESS,
        data,
      });
      await waitForTx(txHash);

      return txHash;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['balance'] });
      queryClient.invalidateQueries({ queryKey: ['recentTransfers'] });
      queryClient.invalidateQueries({ queryKey: ['withdrawalRouting'] });
    },
  }));
}
