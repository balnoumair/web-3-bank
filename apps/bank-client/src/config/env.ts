export const env = {
  tempoRpcUrl: import.meta.env.VITE_TEMPO_RPC_URL || 'http://localhost:8545',
  tempoChainId: Number(import.meta.env.VITE_TEMPO_CHAIN_ID || '1337'),
  tempoExplorerUrl: import.meta.env.VITE_TEMPO_EXPLORER_URL || '',
  bffUrl: import.meta.env.VITE_BFF_URL || 'http://localhost:4000/graphql',
  bankAddress: (import.meta.env.VITE_BANK_ADDRESS || '0x') as `0x${string}`,
  syncUsdAddress: (import.meta.env.VITE_SYNCUSD_ADDRESS || '0x') as `0x${string}`,
  usdcAddress: (import.meta.env.VITE_USDC_ADDRESS || '0x') as `0x${string}`,
} as const;
