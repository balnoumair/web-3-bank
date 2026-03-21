import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { encodeFunctionData } from 'viem';
import { erc20Abi, SYNCUSD_ADDRESS } from '~/config/contracts';
import { signAndSendPasskeyTx, waitForTx } from '~/lib/viem-client';

export function useTransfer(userAddress: () => `0x${string}` | undefined) {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async ({
      to,
      amount,
    }: {
      to: `0x${string}`;
      amount: bigint;
    }) => {
      const from = userAddress();
      if (!from) throw new Error('Not connected');

      const data = encodeFunctionData({
        abi: erc20Abi,
        functionName: 'transfer',
        args: [to, amount],
      });

      const txHash = await signAndSendPasskeyTx({
        from,
        to: SYNCUSD_ADDRESS,
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
