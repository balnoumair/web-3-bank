import type {
  ScoreResult,
  RouteUpdatedOnchain,
  OnchainPublishResult,
} from "@repo/shared-types";

import type { PublishRouteTxReceipt } from "../../src/onchain-publish-adapter";

// ── Input: ScoreResult that feeds the mapper ────────────────────────

export const SCORE_RESULT_FOR_ONCHAIN: ScoreResult = {
  runId: "run-onchain-adapter-001",
  customerId: "customer-onchain-1",
  ranked: [
    {
      chain: "base-sepolia",
      score: 0.82,
      components: { fee: 0.9, latency: 0.7, reliability: 0.85, liquidity: 0.8 },
    },
    {
      chain: "arbitrum-sepolia",
      score: 0.65,
      components: { fee: 0.7, latency: 0.6, reliability: 0.75, liquidity: 0.55 },
    },
  ],
  recommendedChain: "base-sepolia",
  confidence: 0.82,
  reasonCodes: ["SCENARIO_NORMAL"],
};

// ── Expected: mapped onchain payload ────────────────────────────────

/** Expected output when SCORE_RESULT_FOR_ONCHAIN is mapped at epoch 1740000000. */
export const EXPECTED_ONCHAIN_PAYLOAD: RouteUpdatedOnchain = {
  runId: "run-onchain-adapter-001",
  customerId: "customer-onchain-1",
  recommendedChain: "base-sepolia",
  score: 82,
  timestamp: 1740000000,
};

// ── Adapter result cases ────────────────────────────────────────────

export const CONFIRMED_PUBLISH_RESULT: OnchainPublishResult = {
  status: "confirmed",
  txHash: "0xdeadbeef0001",
  blockNumber: 100,
  eventLogIndex: 0,
};

export const DUPLICATE_PUBLISH_RESULT: OnchainPublishResult = {
  status: "duplicateRunId",
  runId: "run-onchain-adapter-001",
};

export const FAILED_PUBLISH_RESULT: OnchainPublishResult = {
  status: "failed",
  error: "execution reverted: RouteReceiver: not authorized publisher",
};

// ── Stub contract call responses ────────────────────────────────────

export const SUCCESSFUL_TX_RECEIPT: PublishRouteTxReceipt = {
  txHash: "0xdeadbeef0001",
  blockNumber: 100,
  logs: [
    {
      eventName: "RoutePublished",
      logIndex: 0,
      args: {
        runId: "run-onchain-adapter-001",
        customerId: "customer-onchain-1",
        recommendedChain: "base-sepolia",
        score: 82n,
        timestamp: 1740000000n,
      },
    },
  ],
};
