import {
  HTTPCapability,
  HTTPClient,
  CronCapability,
  ConsensusAggregationByFields,
  consensusMedianAggregation,
  consensusIdenticalAggregation,
  median,
  identical,
  ok,
  json,
  Runner,
  decodeJson,
  handler,
  type HTTPPayload,
  type CronPayload,
  type HTTPSendRequester,
  type Runtime,
} from "@chainlink/cre-sdk";

// ── Config type for this workflow (loaded from config.staging.json) ──

type ChainEndpointConfig = {
  rpcUrl: string;
  defiLlamaName: string;
};

type EvaluatorConfig = {
  workflowName: string;
  defaultActivationThreshold: number;
  destinationChain: string;
  defiLlamaUrl: string;
  cronSchedule: string;
  active: boolean;
  defaultAllowedChains: string[];
  defaultScenario: "normal" | "congested";
  defaultCustomerId: string;
  chainRegistry: Record<string, ChainEndpointConfig>;
};

// ── Inbound request shape (chainMetrics removed — CRE Evaluator fetches its own) ──

type EvaluationRequest = {
  requestId: string;
  customerId: string;
  scenario: "normal" | "congested";
  allowedChains: string[];
  activationThreshold?: number;
};

// ── Data fetching via CRE SDK HTTPClient ────────────────────────────
// Each function runs inside the DON's node-level execution context
// and is wrapped in consensus aggregation by the calling code.

type RpcGasResult = { feeGwei: number };

/**
 * Fetch gas price from an EVM RPC via JSON-RPC eth_gasPrice.
 * Returns fee in gwei. Used with consensusMedianAggregation.
 */
const fetchGasPrice = (
  sendRequester: HTTPSendRequester,
  rpcUrl: string,
): number => {
  const payload = JSON.stringify({
    jsonrpc: "2.0",
    method: "eth_gasPrice",
    params: [],
    id: 1,
  });
  const body = Buffer.from(new TextEncoder().encode(payload)).toString("base64");

  const response = sendRequester
    .sendRequest({
      url: rpcUrl,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body,
    })
    .result();

  if (!ok(response)) {
    return -1; // sentinel: fetch failed
  }

  const data = json(response) as { result?: string };
  if (!data.result) return -1;

  return parseInt(data.result, 16) / 1e9; // wei → gwei
};

type BlockRecencyResult = { blockAge: number };

/**
 * Fetch latest block and compute recency (age in seconds).
 * Used to derive a reliability score (fresher = more reliable).
 */
const fetchBlockAge = (
  sendRequester: HTTPSendRequester,
  rpcUrl: string,
  nowSeconds: number,
): number => {
  const payload = JSON.stringify({
    jsonrpc: "2.0",
    method: "eth_getBlockByNumber",
    params: ["latest", false],
    id: 2,
  });
  const body = Buffer.from(new TextEncoder().encode(payload)).toString("base64");

  const response = sendRequester
    .sendRequest({
      url: rpcUrl,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body,
    })
    .result();

  if (!ok(response)) {
    return -1;
  }

  const data = json(response) as { result?: { timestamp?: string } };
  if (!data.result?.timestamp) return -1;

  const blockTimestamp = parseInt(data.result.timestamp, 16);
  return Math.abs(nowSeconds - blockTimestamp);
};

type TvlEntry = { name: string; tvl: number };

/**
 * Fetch DeFiLlama chain TVL data (single GET, returns full chain list).
 * Result is consensus-validated across DON nodes via identical aggregation.
 */
const fetchDeFiLlamaTvl = (
  sendRequester: HTTPSendRequester,
  defiLlamaUrl: string,
): string => {
  const response = sendRequester
    .sendRequest({ url: defiLlamaUrl, method: "GET" })
    .result();

  if (!ok(response)) {
    return "[]";
  }

  // Return as string to keep consensus simple (identical string match)
  const data = json(response) as TvlEntry[];
  // Only keep name + tvl to reduce payload size for consensus
  const compact = data.map((e) => ({ n: e.name, t: e.tvl }));
  return JSON.stringify(compact);
};

// ── Metrics assembly ────────────────────────────────────────────────

type ChainMetricsInput = {
  chain: string;
  feeRaw: number | null;
  latencyRaw: number | null;
  reliabilityRaw: number | null;
  liquidityRaw: number | null;
};

/**
 * Fetch all chain metrics using the CRE SDK's HTTPClient with DON consensus.
 * For each allowed chain, fetches gas price and block recency from its RPC,
 * plus DeFiLlama TVL as a single batch call.
 */
function fetchAllChainMetrics(
  runtime: Runtime<EvaluatorConfig>,
  httpClient: HTTPClient,
  allowedChains: string[],
): ChainMetricsInput[] {
  const config = runtime.config;
  const nowSeconds = Math.floor(runtime.now().getTime() / 1000);

  // Fetch DeFiLlama TVL once (identical consensus — all nodes must agree)
  const tvlRaw = httpClient
    .sendRequest(
      runtime,
      fetchDeFiLlamaTvl,
      consensusIdenticalAggregation<string>(),
    )(config.defiLlamaUrl)
    .result();

  const tvlData = JSON.parse(tvlRaw) as Array<{ n: string; t: number }>;
  const tvlMap: Record<string, number> = {};
  for (const entry of tvlData) {
    tvlMap[entry.n] = entry.t;
  }

  // Find max TVL for normalisation to 0-1 range
  const allTvls = Object.values(tvlMap);
  const maxTvl = allTvls.length > 0 ? Math.max(...allTvls) : 1;

  // Fetch per-chain metrics
  return allowedChains.map((chain) => {
    const endpoint = config.chainRegistry[chain];
    if (!endpoint) {
      runtime.log(`Chain "${chain}" not found in chainRegistry, skipping fetch`);
      return {
        chain,
        feeRaw: null,
        latencyRaw: null,
        reliabilityRaw: null,
        liquidityRaw: null,
      };
    }

    // Gas price with median consensus (tolerates minor differences between nodes)
    const gasPriceGwei = httpClient
      .sendRequest(
        runtime,
        fetchGasPrice,
        consensusMedianAggregation<number>(),
      )(endpoint.rpcUrl)
      .result();

    // Block age with median consensus
    const blockAge = httpClient
      .sendRequest(
        runtime,
        fetchBlockAge,
        consensusMedianAggregation<number>(),
      )(endpoint.rpcUrl, nowSeconds)
      .result();

    // Convert to scoring inputs
    const feeRaw = gasPriceGwei >= 0 ? gasPriceGwei : null;

    // Reliability: block age → score (0-1). Fresh block (< 5s) = 1.0, stale (> 120s) = 0.0
    const reliabilityRaw =
      blockAge >= 0 ? Math.max(0, Math.min(1, 1 - blockAge / 120)) : null;

    // Liquidity from DeFiLlama TVL, normalised to 0-1 vs max
    const chainTvl = tvlMap[endpoint.defiLlamaName];
    const liquidityRaw =
      chainTvl !== undefined && maxTvl > 0 ? chainTvl / maxTvl : null;

    // Latency: cannot be measured in DON mode (each node has its own latency)
    // The scoring fallback mechanism handles null values
    const latencyRaw = null;

    return { chain, feeRaw, latencyRaw, reliabilityRaw, liquidityRaw };
  });
}

// ── Scoring (inline deterministic implementation) ───────────────────
// Inlined because the CRE SDK compiles to WASM — workspace imports
// from @repo/cre-workflows are not available at WASM runtime.

const WEIGHTS = { fee: 0.35, latency: 0.3, reliability: 0.25, liquidity: 0.1 };
const METRIC_KEYS = ["feeRaw", "latencyRaw", "reliabilityRaw", "liquidityRaw"] as const;
const DIRECTION: Record<string, "lower-better" | "higher-better"> = {
  feeRaw: "lower-better",
  latencyRaw: "lower-better",
  reliabilityRaw: "higher-better",
  liquidityRaw: "higher-better",
};
const COMPONENT_KEY: Record<string, string> = {
  feeRaw: "fee",
  latencyRaw: "latency",
  reliabilityRaw: "reliability",
  liquidityRaw: "liquidity",
};
const FALLBACKS: Record<string, Record<string, number>> = {
  normal: { feeRaw: 35, latencyRaw: 220, reliabilityRaw: 0.965, liquidityRaw: 0.75 },
  congested: { feeRaw: 85, latencyRaw: 480, reliabilityRaw: 0.9, liquidityRaw: 0.55 },
};
const TIE_EPSILON = 1e-9;
const DECIMALS = 6;

function roundN(v: number, d: number): number {
  const f = 10 ** d;
  return Math.round(v * f) / f;
}

type RankedEntry = {
  chain: string;
  score: number;
  components: Record<string, number>;
};

function scoreChains(
  scenario: string,
  allowedChains: string[],
  metrics: ChainMetricsInput[],
): { ranked: RankedEntry[]; reasonCodes: string[] } {
  const reasons: string[] = [];
  reasons.push(scenario === "congested" ? "SCENARIO_CONGESTED" : "SCENARIO_NORMAL");

  const fb = FALLBACKS[scenario] ?? FALLBACKS.normal;

  // Build completed metrics with fallbacks
  const completed = allowedChains.map((chain) => {
    const found = metrics.find((m) => m.chain === chain);
    const vals: Record<string, number> = {};
    for (const key of METRIC_KEYS) {
      const raw = found ? (found as Record<string, unknown>)[key] : null;
      if (typeof raw === "number" && Number.isFinite(raw)) {
        vals[key] = raw;
      } else {
        vals[key] = fb[key];
        reasons.push(`MISSING_${key.replace("Raw", "").toUpperCase()}_RAW_FALLBACK`);
      }
    }
    return { chain, ...vals };
  });

  // Normalise and score
  const scored: RankedEntry[] = completed.map((c) => {
    const comps: Record<string, number> = {};
    let total = 0;
    for (const key of METRIC_KEYS) {
      const values = completed.map((x) => (x as unknown as Record<string, number>)[key]);
      const min = Math.min(...values);
      const max = Math.max(...values);
      const range = max - min;
      let norm: number;
      if (range < TIE_EPSILON) {
        norm = 1;
        reasons.push(`ZERO_RANGE_${key.replace("Raw", "").toUpperCase()}_RAW`);
      } else {
        const raw = (c as unknown as Record<string, number>)[key];
        norm = DIRECTION[key] === "lower-better"
          ? (max - raw) / range
          : (raw - min) / range;
      }
      const weight = (WEIGHTS as Record<string, number>)[COMPONENT_KEY[key]];
      const weighted = roundN(norm * weight, DECIMALS);
      comps[COMPONENT_KEY[key]] = weighted;
      total += weighted;
    }
    return { chain: c.chain, score: roundN(total, DECIMALS), components: comps };
  });

  // Sort descending by score, then tie-break
  scored.sort((a, b) => {
    const diff = b.score - a.score;
    if (Math.abs(diff) > TIE_EPSILON) return diff;
    // Tie-break: reliability desc, latency asc, chain lex
    const aRel = a.components.reliability ?? 0;
    const bRel = b.components.reliability ?? 0;
    if (Math.abs(bRel - aRel) > TIE_EPSILON) return bRel - aRel;
    const aLat = a.components.latency ?? 0;
    const bLat = b.components.latency ?? 0;
    if (Math.abs(aLat - bLat) > TIE_EPSILON) return aLat - bLat;
    return a.chain.localeCompare(b.chain);
  });

  return { ranked: scored, reasonCodes: [...new Set(reasons)] };
}

function classifyActivation(
  threshold: number,
  ranked: RankedEntry[],
): { activeChains: string[]; inactiveChains: string[] } {
  const active: string[] = [];
  const inactive: string[] = [];
  for (const entry of ranked) {
    if (entry.score >= threshold) {
      active.push(entry.chain);
    } else {
      inactive.push(entry.chain);
    }
  }
  return { activeChains: active, inactiveChains: inactive };
}

// ── Response types ──────────────────────────────────────────────────

type EvaluationResponse = {
  requestId: string;
  runId: string;
  customerId: string;
  recommendedChain: string;
  confidence: number;
  ranked: RankedEntry[];
  activation: {
    thresholdUsed: number;
    activeChains: string[];
    inactiveChains: string[];
  };
  reasonCodes: string[];
  evaluatedAt: string;
};

type EvaluationErrorResponse = {
  errorCode: string;
  errorMessage: string;
  requestId: string | null;
};

// ── Validation ──────────────────────────────────────────────────────

function isValidRequest(payload: unknown): payload is EvaluationRequest {
  if (typeof payload !== "object" || payload === null) return false;
  const obj = payload as Record<string, unknown>;
  return (
    typeof obj.requestId === "string" &&
    typeof obj.customerId === "string" &&
    (obj.scenario === "normal" || obj.scenario === "congested") &&
    Array.isArray(obj.allowedChains) &&
    obj.allowedChains.length > 0
  );
}

// ── Run ID generation ───────────────────────────────────────────────

let runSequence = 0;

function generateRunId(): string {
  runSequence += 1;
  return `run-${Date.now()}-${runSequence}`;
}

// ── HTTPClient instance (shared across handler invocations) ─────────

const httpClient = new HTTPClient();

// ── Core evaluation logic (shared by HTTP and Cron triggers) ────────

function evaluateChains(
  runtime: Runtime<EvaluatorConfig>,
  request: EvaluationRequest,
): string {
  // Validate allowed chains against registry
  const unknownChains = request.allowedChains.filter(
    (chain) => !(chain in runtime.config.chainRegistry),
  );
  if (unknownChains.length > 0) {
    runtime.log(
      `Warning: chains not in registry will use fallback metrics: ${unknownChains.join(", ")}`,
    );
  }

  const runId = generateRunId();
  runtime.log(`CRE Evaluator starting scoring run ${runId} for customer ${request.customerId}`);

  // Fetch chain metrics via HTTPClient (DON consensus-validated)
  runtime.log(`CRE Evaluator fetching chain metrics for: ${request.allowedChains.join(", ")}`);
  const chainMetrics = fetchAllChainMetrics(runtime, httpClient, request.allowedChains);
  runtime.log(`CRE Evaluator metrics fetched: ${JSON.stringify(chainMetrics)}`);

  // Score chains
  const { ranked, reasonCodes } = scoreChains(
    request.scenario,
    request.allowedChains,
    chainMetrics,
  );

  const recommendedChain = ranked.length > 0 ? ranked[0].chain : "none";
  const confidence = ranked.length > 0 ? ranked[0].score : 0;

  // Classify activation
  const threshold =
    typeof request.activationThreshold === "number" &&
      request.activationThreshold >= 0 &&
      request.activationThreshold <= 1
      ? request.activationThreshold
      : runtime.config.defaultActivationThreshold;

  const activation = classifyActivation(threshold, ranked);

  runtime.log(
    `CRE Evaluator scoring complete: recommended=${recommendedChain} score=${confidence} ` +
    `active=[${activation.activeChains.join(",")}] inactive=[${activation.inactiveChains.join(",")}] ` +
    `threshold=${threshold}`,
  );

  const response: EvaluationResponse = {
    requestId: request.requestId,
    runId,
    customerId: request.customerId,
    recommendedChain,
    confidence,
    ranked,
    activation: {
      thresholdUsed: threshold,
      ...activation,
    },
    reasonCodes,
    evaluatedAt: new Date().toISOString(),
  };

  return JSON.stringify(response);
}

// ── HTTP Handler (on-demand evaluation) ─────────────────────────────

const onEvaluate = (
  runtime: Runtime<EvaluatorConfig>,
  payload: HTTPPayload,
): string => {
  const raw = decodeJson(payload.input) as Record<string, unknown>;
  runtime.log(`CRE Evaluator evaluation request received: ${JSON.stringify(raw)}`);

  // Validate
  if (!isValidRequest(raw)) {
    const errorResponse: EvaluationErrorResponse = {
      errorCode: "INVALID_PAYLOAD",
      errorMessage:
        "Missing or invalid fields: requestId, customerId, scenario (normal|congested), allowedChains (min 1) are required",
      requestId: typeof raw.requestId === "string" ? raw.requestId : null,
    };
    runtime.log(`CRE Evaluator validation failed: ${errorResponse.errorMessage}`);
    return JSON.stringify(errorResponse);
  }

  return evaluateChains(runtime, raw);
};

// ── Cron Handler (scheduled evaluation) ─────────────────────────────

const onCronEvaluate = (
  runtime: Runtime<EvaluatorConfig>,
  payload: CronPayload,
): string => {
  // Kill-switch: skip evaluation if config says inactive
  if (!runtime.config.active) {
    runtime.log("CRE Evaluator cron tick skipped: active=false");
    return JSON.stringify({ skipped: true, reason: "inactive" });
  }

  runtime.log(
    `CRE Evaluator cron tick at ${String(payload.scheduledExecutionTime ?? "unknown")}`,
  );

  // Build request from config defaults (no inbound HTTP payload)
  const request: EvaluationRequest = {
    requestId: `cron-${Date.now()}`,
    customerId: runtime.config.defaultCustomerId,
    scenario: runtime.config.defaultScenario,
    allowedChains: runtime.config.defaultAllowedChains,
    activationThreshold: runtime.config.defaultActivationThreshold,
  };

  return evaluateChains(runtime, request);
};

// ── Workflow initialisation (dual-trigger: HTTP + Cron) ─────────────

const initWorkflow = () => {
  const http = new HTTPCapability();
  const cron = new CronCapability();
  return [
    handler(http.trigger({}), onEvaluate),
    handler(cron.trigger({ schedule: "*/5 * * * *" }), onCronEvaluate),
  ];
};

export async function main() {
  const runner = await Runner.newRunner<EvaluatorConfig>();
  await runner.run(initWorkflow);
}
