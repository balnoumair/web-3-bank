import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { encodeFunctionData } from 'viem';
import { bankAbi, BANK_ADDRESS, erc20Abi, SYNCUSD_ADDRESS } from '~/config/contracts';
import { env } from '~/config/env';
import { signAndSendPasskeyTx, waitForTx } from '~/lib/viem-client';

export function useTransfer(userAddress: () => `0x${string}` | undefined) {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async ({
      to,
      amount,
      destChainId,
    }: {
      to: `0x${string}`;
      amount: bigint;
      destChainId: string;
    }) => {
      const from = userAddress();
      if (!from) throw new Error('Not connected');

      const destinationChainId = BigInt(destChainId);
      const senderChainId = BigInt(env.tempoChainId);

      const data =
        destinationChainId === senderChainId
          ? encodeFunctionData({
              abi: erc20Abi,
              functionName: 'transfer',
              args: [to, amount],
            })
          : encodeFunctionData({
              abi: bankAbi,
              functionName: 'transferHotPath',
              args: [to, amount, destinationChainId],
            });

      const contract = destinationChainId === senderChainId ? SYNCUSD_ADDRESS : BANK_ADDRESS;

      const txHash = await signAndSendPasskeyTx({
        from,
        to: contract,
        data,
      });
      await waitForTx(txHash);

      return txHash;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['balance'] });
      queryClient.invalidateQueries({ queryKey: ['recentTransfers'] });
    },
  }));
}
