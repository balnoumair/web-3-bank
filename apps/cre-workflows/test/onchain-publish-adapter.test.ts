import { describe, expect, it } from "vitest";
import type { SimulationRequest } from "@repo/shared-types";

import {
  toRouteUpdatedOnchain,
  createOnchainPublishAdapter,
  type PublishRouteTxReceipt,
} from "../src";

import {
  SCORE_RESULT_FOR_ONCHAIN,
  EXPECTED_ONCHAIN_PAYLOAD,
  SUCCESSFUL_TX_RECEIPT,
} from "./fixtures/onchain-publish-cases";

// ── toRouteUpdatedOnchain mapper ────────────────────────────────────

describe("toRouteUpdatedOnchain", () => {
  it("maps ScoreResult to RouteUpdatedOnchain with integer score and epoch timestamp", () => {
    const result = toRouteUpdatedOnchain(SCORE_RESULT_FOR_ONCHAIN, 1740000000);

    expect(result).toEqual(EXPECTED_ONCHAIN_PAYLOAD);
  });

  it("scales a fractional score to integer by multiplying by 100 and flooring", () => {
    const result = toRouteUpdatedOnchain(
      { ...SCORE_RESULT_FOR_ONCHAIN, ranked: [
        { chain: "base-sepolia", score: 0.8567, components: { fee: 0, latency: 0, reliability: 0, liquidity: 0 } },
      ] },
      1740000000,
    );

    expect(result.score).toBe(85);
  });

  it("uses 0 score when recommendedChain is not found in ranked", () => {
    const result = toRouteUpdatedOnchain(
      { ...SCORE_RESULT_FOR_ONCHAIN, recommendedChain: "nonexistent-chain" },
      1740000000,
    );

    expect(result.score).toBe(0);
  });

  it("preserves runId, customerId, and recommendedChain verbatim", () => {
    const result = toRouteUpdatedOnchain(SCORE_RESULT_FOR_ONCHAIN, 1740000000);

    expect(result.runId).toBe(SCORE_RESULT_FOR_ONCHAIN.runId);
    expect(result.customerId).toBe(SCORE_RESULT_FOR_ONCHAIN.customerId);
    expect(result.recommendedChain).toBe(SCORE_RESULT_FOR_ONCHAIN.recommendedChain);
  });
});

// ── createOnchainPublishAdapter ─────────────────────────────────────

describe("createOnchainPublishAdapter", () => {
  it("returns confirmed result with txHash, blockNumber, and eventLogIndex on success", async () => {
    const adapter = createOnchainPublishAdapter({
      publishRoute: async () => SUCCESSFUL_TX_RECEIPT,
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(result).toEqual({
      status: "confirmed",
      txHash: "0xdeadbeef0001",
      blockNumber: 100,
      eventLogIndex: 0,
    });
  });

  it("returns duplicateRunId when contract reverts with 'runId already published'", async () => {
    const adapter = createOnchainPublishAdapter({
      publishRoute: async () => {
        throw new Error("execution reverted: RouteReceiver: runId already published");
      },
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(result).toEqual({
      status: "duplicateRunId",
      runId: "run-onchain-adapter-001",
    });
  });

  it("runs simulation first and attaches simulation evidence to confirmed result", async () => {
    const simulationCalls: SimulationRequest[] = [];

    const adapter = createOnchainPublishAdapter({
      simulationPort: {
        simulate: async (req) => {
          simulationCalls.push(req);
          return {
            success: true,
            simulationId: "sim-onchain-001",
            traceUrl:
              "https://dashboard.tenderly.co/account/project/project/simulator/sim-onchain-001",
            gasUsed: 210000,
            errorMessage: null,
          };
        },
      },
      buildSimulationRequest: () => ({
        networkId: "84532",
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        input: "0xabcdef",
        gas: 300000,
        value: 0,
      }),
      publishRoute: async () => SUCCESSFUL_TX_RECEIPT,
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(simulationCalls).toEqual([
      {
        networkId: "84532",
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        input: "0xabcdef",
        gas: 300000,
        value: 0,
      },
    ]);

    expect(result).toEqual({
      status: "confirmed",
      txHash: "0xdeadbeef0001",
      blockNumber: 100,
      eventLogIndex: 0,
      simulationId: "sim-onchain-001",
      simulationTraceUrl:
        "https://dashboard.tenderly.co/account/project/project/simulator/sim-onchain-001",
    });
  });

  it("returns failed and skips publish when simulation predicts failure", async () => {
    let publishCalled = false;

    const adapter = createOnchainPublishAdapter({
      simulationPort: {
        simulate: async () => ({
          success: false,
          simulationId: "sim-failed-001",
          traceUrl:
            "https://dashboard.tenderly.co/account/project/project/simulator/sim-failed-001",
          gasUsed: 45000,
          errorMessage: "execution reverted: unauthorized publisher",
        }),
      },
      buildSimulationRequest: () => ({
        networkId: "84532",
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        input: "0xabcdef",
        gas: 300000,
        value: 0,
      }),
      publishRoute: async () => {
        publishCalled = true;
        return SUCCESSFUL_TX_RECEIPT;
      },
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(publishCalled).toBe(false);
    expect(result).toEqual({
      status: "failed",
      error: "execution reverted: unauthorized publisher",
    });
  });

  it("returns failed when simulation port is provided without a request builder", async () => {
    const adapter = createOnchainPublishAdapter({
      simulationPort: {
        simulate: async () => ({
          success: true,
          simulationId: "sim-no-builder",
          traceUrl:
            "https://dashboard.tenderly.co/account/project/project/simulator/sim-no-builder",
          gasUsed: 1000,
          errorMessage: null,
        }),
      },
      publishRoute: async () => SUCCESSFUL_TX_RECEIPT,
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(result).toEqual({
      status: "failed",
      error: "Simulation requested but buildSimulationRequest is missing",
    });
  });

  it("returns failed result for non-duplicate revert errors", async () => {
    const adapter = createOnchainPublishAdapter({
      publishRoute: async () => {
        throw new Error("execution reverted: RouteReceiver: not authorized publisher");
      },
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(result).toEqual({
      status: "failed",
      error: "execution reverted: RouteReceiver: not authorized publisher",
    });
  });

  it("returns failed result for provider/network errors", async () => {
    const adapter = createOnchainPublishAdapter({
      publishRoute: async () => {
        throw new Error("network timeout");
      },
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(result).toEqual({
      status: "failed",
      error: "network timeout",
    });
  });

  it("passes all payload fields to the contract call function", async () => {
    const calls: Array<{
      runId: string;
      customerId: string;
      recommendedChain: string;
      score: number;
      timestamp: number;
    }> = [];

    const adapter = createOnchainPublishAdapter({
      publishRoute: async (payload) => {
        calls.push(payload);
        return SUCCESSFUL_TX_RECEIPT;
      },
    });

    await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(calls).toEqual([
      {
        runId: "run-onchain-adapter-001",
        customerId: "customer-onchain-1",
        recommendedChain: "base-sepolia",
        score: 82,
        timestamp: 1740000000,
      },
    ]);
  });

  it("extracts event data from the first RoutePublished log entry", async () => {
    const receiptWithMultipleLogs: PublishRouteTxReceipt = {
      ...SUCCESSFUL_TX_RECEIPT,
      logs: [
        ...SUCCESSFUL_TX_RECEIPT.logs,
        {
          eventName: "RoutePublished",
          logIndex: 3,
          args: {
            runId: "run-onchain-adapter-002",
            customerId: "customer-2",
            recommendedChain: "arbitrum-sepolia",
            score: 65n,
            timestamp: 1740000001n,
          },
        },
      ],
    };

    const adapter = createOnchainPublishAdapter({
      publishRoute: async () => receiptWithMultipleLogs,
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(result).toEqual({
      status: "confirmed",
      txHash: "0xdeadbeef0001",
      blockNumber: 100,
      eventLogIndex: 0,
    });
  });

  it("returns 'unknown publish error' when a non-Error value is thrown", async () => {
    const adapter = createOnchainPublishAdapter({
      publishRoute: async () => {
        throw "string error, not an Error instance";
      },
    });

    const result = await adapter.publish(EXPECTED_ONCHAIN_PAYLOAD);

    expect(result).toEqual({
      status: "failed",
      error: "unknown publish error",
    });
  });
});
