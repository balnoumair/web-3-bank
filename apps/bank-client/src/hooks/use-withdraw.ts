import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { encodeFunctionData } from 'viem';
import { bankAbi, BANK_ADDRESS, USDC_ADDRESS } from '~/config/contracts';
import { signAndSendPasskeyTx, waitForTx } from '~/lib/viem-client';

export function useWithdraw(userAddress: () => `0x${string}` | undefined) {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async ({ amount }: { amount: bigint }) => {
      const from = userAddress();
      if (!from) throw new Error('Not connected');

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
    },
  }));
}
