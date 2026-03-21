export const POOL_DEPTHS_QUERY = `
  query PoolDepths($chainId: Int!) {
    poolDepths(chainId: $chainId) {
      chainId
      depthWei
    }
  }
`;

export interface PoolDepth {
  chainId: string;
  depthWei: string;
}

export interface PoolDepthsResponse {
  poolDepths: PoolDepth;
}
