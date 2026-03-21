import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { encodeFunctionData } from 'viem';
import {
  bankAbi,
  erc20Abi,
  BANK_ADDRESS,
  USDC_ADDRESS,
} from '~/config/contracts';
import { publicClient, signAndSendPasskeyTx, waitForTx } from '~/lib/viem-client';

export function useDeposit(userAddress: () => `0x${string}` | undefined) {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async ({ amount }: { amount: bigint }) => {
      const from = userAddress();
      if (!from) throw new Error('Not connected');

      // Check USDC allowance for Bank contract
      const allowance = await publicClient.readContract({
        address: USDC_ADDRESS,
        abi: erc20Abi,
        functionName: 'allowance',
        args: [from, BANK_ADDRESS],
      });

      // Approve if insufficient allowance
      if (allowance < amount) {
        const approveData = encodeFunctionData({
          abi: erc20Abi,
          functionName: 'approve',
          args: [BANK_ADDRESS, amount],
        });
        const approveTx = await signAndSendPasskeyTx({
          from,
          to: USDC_ADDRESS,
          data: approveData,
        });
        await waitForTx(approveTx);
      }

      // Execute deposit
      const depositData = encodeFunctionData({
        abi: bankAbi,
        functionName: 'deposit',
        args: [USDC_ADDRESS, amount],
      });
      const txHash = await signAndSendPasskeyTx({
        from,
        to: BANK_ADDRESS,
        data: depositData,
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
