import { createMutation, useQueryClient } from '@tanstack/solid-query';
import { encodeFunctionData } from 'viem';
import {
  bankAbi,
  erc20Abi,
  BANK_ADDRESS,
  SYNCUSD_ADDRESS,
} from '~/config/contracts';
import { publicClient, signAndSendPasskeyTx, waitForTx } from '~/lib/viem-client';

export function useTransferCrossChain(
  userAddress: () => `0x${string}` | undefined,
) {
  const queryClient = useQueryClient();

  return createMutation(() => ({
    mutationFn: async ({
      to,
      amount,
      destinationChainId,
    }: {
      to: `0x${string}`;
      amount: bigint;
      destinationChainId: bigint;
    }) => {
      const from = userAddress();
      if (!from) throw new Error('Not connected');

      // Check SyncUSD allowance for Bank contract
      const allowance = await publicClient.readContract({
        address: SYNCUSD_ADDRESS,
        abi: erc20Abi,
        functionName: 'allowance',
        args: [from, BANK_ADDRESS],
      });

      // Approve if insufficient
      if (allowance < amount) {
        const approveData = encodeFunctionData({
          abi: erc20Abi,
          functionName: 'approve',
          args: [BANK_ADDRESS, amount],
        });
        const approveTx = await signAndSendPasskeyTx({
          from,
          to: SYNCUSD_ADDRESS,
          data: approveData,
        });
        await waitForTx(approveTx);
      }

      // Execute cross-chain transfer
      const data = encodeFunctionData({
        abi: bankAbi,
        functionName: 'transferHotPath',
        args: [to, amount, destinationChainId],
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
