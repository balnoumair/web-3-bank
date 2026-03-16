# CRE Evaluator Evaluator

The primary evaluator workflow in the CRE Route Orchestrator stack. CRE Evaluator receives an evaluation request from CRE Policy specifying multiple chain paths and selects the optimal path based on real-time multi-dimensional scoring.

## Self-Fetching Architecture

As of the "CRE Evaluator Self-Fetching" refactoring, CRE Evaluator is fully decentralized and autonomous. It does **not** rely on external agents for data provision. 

Instead of receiving pre-fetched data from CRE Policy or the client, CRE Evaluator leverages the native CRE SDK `HTTPClient` with **DON Consensus Aggregation** to fetch data dynamically at execution time.

### How Data is Fetched

1. **RPC Queries**: Gas prices (`eth_gasPrice`) and block recency (`eth_getBlockByNumber`) are fetched individually for each requested chain via its configured JSON-RPC node. 
   - Uses `consensusMedianAggregation` to ensure all oracle nodes agree on a reasonably similar value, protecting against outlier nodes.
2. **DeFiLlama API**: Liquidity metrics (TVL) are fetched from the DeFiLlama chains endpoint in a single batch request.
   - Uses `consensusIdenticalAggregation` to mandate identical data mapping across all node responses.

### Chain Registry Configuration

CRE Evaluator manages its own `chainRegistry` inside `config.staging.json` (and production equivalents). When CRE Policy submits `allowedChains`, CRE Evaluator resolves their specific endpoints internally.

```json
{
  "chainRegistry": {
    "base-sepolia": { "rpcUrl": "...", "defiLlamaName": "Base" },
    "arbitrum-sepolia": { "rpcUrl": "...", "defiLlamaName": "Arbitrum" }
  }
}
```

### The Latency Trade-Off

By moving outbound requests inside the DON (Decentralized Oracle Network), **latency ping testing is no longer viable**. Each node executes HTTP requests independently from its own geographical and network position, meaning there is no single "round-trip time" to form a consensus over. 

CRE Evaluator sets `latencyRaw: null` for all chains, allowing the scoring engine's native fallback mechanics to trigger. The latency weight is preserved (30%) but relies entirely on scenario defaults (220ms for normal, 480ms for congested) to maintain formula stability without unfairly punishing geographically distant nodes.
