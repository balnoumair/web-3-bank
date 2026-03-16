import { describe, expect, it } from "vitest";

import {
  bindRunStartConfigSnapshot,
  scoreRunV1,
} from "../src";

describe("CRE Evaluator run-start snapshot contract", () => {
  it("reads config snapshot exactly once at run start", async () => {
    let readCount = 0;

    const result = await bindRunStartConfigSnapshot({
      runId: "run-35-read-once",
      customerId: "customer-35",
      hasActiveRun: false,
      timestamp: "2026-02-22T00:00:00.000Z",
      readConfigSnapshot: async () => {
        readCount += 1;

        return {
          customerId: "customer-35",
          configVersion: 7,
          updatedAt: "2026-02-21T23:59:00.000Z",
          allowedChains: ["base-sepolia", "arbitrum-sepolia"],
          active: true,
          scenario: "normal",
        };
      },
    });

    expect(result.status).toBe("started");
    expect(readCount).toBe(1);
  });

  it("keeps active run behavior stable when config changes mid-run", async () => {
    const currentSnapshot = {
      customerId: "customer-35",
      configVersion: 3,
      updatedAt: "2026-02-22T00:00:00.000Z",
      allowedChains: ["base-sepolia"],
      active: true,
      scenario: "normal" as const,
    };

    const binding = await bindRunStartConfigSnapshot({
      runId: "run-35-stable",
      customerId: "customer-35",
      hasActiveRun: false,
      timestamp: "2026-02-22T00:01:00.000Z",
      readConfigSnapshot: async () => currentSnapshot,
    });

    if (binding.status !== "started") {
      throw new Error("expected started binding");
    }

    currentSnapshot.configVersion = 4;
    currentSnapshot.allowedChains = ["arbitrum-sepolia"];
    currentSnapshot.scenario = "congested";

    const score = scoreRunV1({
      runId: "run-35-stable",
      customerId: "customer-35",
      scenario: binding.snapshot.scenario,
      allowedChains: binding.snapshot.allowedChains,
      chainMetrics: [
        {
          chain: "base-sepolia",
          feeRaw: 20,
          latencyRaw: 160,
          reliabilityRaw: 0.99,
          liquidityRaw: 0.88,
        },
        {
          chain: "arbitrum-sepolia",
          feeRaw: 15,
          latencyRaw: 140,
          reliabilityRaw: 0.98,
          liquidityRaw: 0.85,
        },
      ],
    });

    expect(binding.snapshot.configVersion).toBe(3);
    expect(binding.snapshot.allowedChains).toEqual(["base-sepolia"]);
    expect(binding.snapshot.scenario).toBe("normal");
    expect(score.ranked.map((entry) => entry.chain)).toEqual(["base-sepolia"]);
  });

  it("includes snapshot version metadata in run start log", async () => {
    const binding = await bindRunStartConfigSnapshot({
      runId: "run-35-log-meta",
      customerId: "customer-35",
      hasActiveRun: false,
      timestamp: "2026-02-22T00:02:00.000Z",
      readConfigSnapshot: async () => ({
        customerId: "customer-35",
        configVersion: 11,
        updatedAt: "2026-02-22T00:01:00.000Z",
        allowedChains: ["base-sepolia"],
        active: true,
        scenario: "normal",
      }),
    });

    if (binding.status !== "started") {
      throw new Error("expected started binding");
    }

    expect(binding.log.snapshotVersion).toBe(11);
    expect(binding.log.snapshotUpdatedAt).toBe("2026-02-22T00:01:00.000Z");
    expect(binding.log.event).toBe("run.start.accepted");
  });

  it("maps missing snapshot to partialFailure CONFIG_NOT_FOUND", async () => {
    const binding = await bindRunStartConfigSnapshot({
      runId: "run-41-missing",
      customerId: "customer-41-missing",
      hasActiveRun: false,
      timestamp: "2026-02-22T00:03:00.000Z",
      readConfigSnapshot: async () => null,
    });

    expect(binding).toEqual({
      status: "partialFailure",
      errorCode: "CONFIG_NOT_FOUND",
      errorMessage: "config snapshot not found",
      log: {
        level: "warn",
        event: "run.transition.rejected",
        runId: "run-41-missing",
        customerId: "customer-41-missing",
        timestamp: "2026-02-22T00:03:00.000Z",
        fromStatus: null,
        toStatus: "partialFailure",
        errorCode: "CONFIG_NOT_FOUND",
        errorMessage: "config snapshot not found",
      },
    });
  });

  it("maps inactive snapshot to partialFailure CONFIG_INACTIVE", async () => {
    const binding = await bindRunStartConfigSnapshot({
      runId: "run-41-inactive",
      customerId: "customer-41-inactive",
      hasActiveRun: false,
      timestamp: "2026-02-22T00:04:00.000Z",
      readConfigSnapshot: async () => ({
        customerId: "customer-41-inactive",
        configVersion: 8,
        updatedAt: "2026-02-22T00:03:30.000Z",
        allowedChains: ["base-sepolia"],
        active: false,
        scenario: "normal",
      }),
    });

    expect(binding).toEqual({
      status: "partialFailure",
      errorCode: "CONFIG_INACTIVE",
      errorMessage: "config snapshot is inactive",
      log: {
        level: "warn",
        event: "run.transition.rejected",
        runId: "run-41-inactive",
        customerId: "customer-41-inactive",
        timestamp: "2026-02-22T00:04:00.000Z",
        fromStatus: null,
        toStatus: "partialFailure",
        errorCode: "CONFIG_INACTIVE",
        errorMessage: "config snapshot is inactive",
      },
    });
  });
});
