import { defineChain } from 'viem';
import { env } from './env';

export const tempo = defineChain({
  id: env.tempoChainId,
  name: 'Tempo',
  nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
  rpcUrls: {
    default: { http: [env.tempoRpcUrl] },
  },
  blockExplorers: env.tempoExplorerUrl
    ? { default: { name: 'Tempo Explorer', url: env.tempoExplorerUrl } }
    : undefined,
});
