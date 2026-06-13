import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { encodeFunctionData } from 'viem';
import { bankAbi, BANK_ADDRESS, USDC_ADDRESS } from '~/config/contracts';
import { env } from '~/config/env';
import { gql } from '~/lib/graphql';
import { signAndSendPasskeyTx, waitForTx } from '~/lib/viem-client';
import {
  selectWithdrawalChain,
  withdrawalUnavailableMessage,
  WITHDRAWAL_ROUTING_QUERY,
  type WithdrawalRoutingResponse,
} from '~/queries/withdrawal-routing';

export function useWithdraw(userAddress: () => `0x${string}` | undefined) {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async ({ amount }: { amount: bigint }) => {
      const from = userAddress();
      if (!from) throw new Error('Not connected');

      const routing = await gql<WithdrawalRoutingResponse>(WITHDRAWAL_ROUTING_QUERY);
      const selected = selectWithdrawalChain(
        routing.withdrawalRouting,
        amount,
        env.tempoChainId,
      );

      if (!selected) {
        throw new Error(withdrawalUnavailableMessage(routing.withdrawalRouting));
      }

      if (BigInt(selected.chainId) !== BigInt(env.tempoChainId)) {
        throw new Error(
          'Withdrawal is only available on another chain right now. Multi-chain signing is not yet supported in this client.',
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
