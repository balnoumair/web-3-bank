import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import type { PublishRouteTxReceipt } from "../src/onchain-publish-adapter";
import {
  createOnchainPublishAdapter,
  classifyActivationByThreshold,
  toRouteUpdatedOnchain,
  scoreRunV1,
} from "../src";

type NetworkMode = "local" | "base-sepolia";

type CommandOptions = {
  cwd?: string;
  env?: Record<string, string | undefined>;
  allowFailure?: boolean;
};

type CommandResult = {
  code: number;
  stdout: string;
  stderr: string;
};

const REPO_ROOT = process.cwd();
const FOUNDRY_DIR = `${REPO_ROOT}/packages/onchain/foundry`;

const ANVIL_DEFAULT_PRIVATE_KEY =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const ANVIL_SECOND_PRIVATE_KEY =
  "0x7c8521182946c6d46f9f0f01f2f84f8fbfce66a91f4d52f1f0b3f7a2c9b5b6e7";

async function runCommand(
  command: string,
  args: string[],
  options: CommandOptions = {},
): Promise<CommandResult> {
  const child = spawn(command, args, {
    cwd: options.cwd,
    env: {
      ...process.env,
      ...options.env,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";

  child.stdout.on("data", (chunk) => {
    stdout += String(chunk);
  });

  child.stderr.on("data", (chunk) => {
    stderr += String(chunk);
  });

  const [code] = (await once(child, "close")) as [number];
  const exitCode = code ?? 1;

  if (exitCode !== 0 && !options.allowFailure) {
    throw new Error(
      [
        `Command failed: ${command} ${args.join(" ")}`,
        stdout.trim(),
        stderr.trim(),
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }

  return {
    code: exitCode,
    stdout,
    stderr,
  };
}

async function assertCommandAvailable(command: string): Promise<void> {
  const result = await runCommand("bash", ["-lc", `command -v ${command}`], {
    allowFailure: true,
  });

  if (result.code !== 0) {
    throw new Error(`${command} is not installed or not on PATH`);
  }
}

function parseNetwork(): NetworkMode {
  const isBaseSepolia = process.argv.some(arg => arg.includes("base-sepolia"));
  if (isBaseSepolia) {
    return "base-sepolia";
  }

  return "local";
}

function parseReceiverAddress(output: string): string {
  const addressMatch = output.match(/Deployed RouteReceiver at:\s*(0x[a-fA-F0-9]{40})/);
  if (!addressMatch) {
    throw new Error("Could not parse deployed receiver address from forge output");
  }

  return addressMatch[1];
}

function parseHexNumber(value: unknown): number {
  if (typeof value === "number") {
    return value;
  }

  if (typeof value === "string") {
    return value.startsWith("0x") ? Number.parseInt(value, 16) : Number.parseInt(value, 10);
  }

  return 0;
}

async function deployReceiver(params: {
  rpcUrl: string;
  privateKey: string;
}): Promise<string> {
  const result = await runCommand(
    "forge",
    [
      "script",
      "script/Deploy.s.sol",
      "--rpc-url",
      params.rpcUrl,
      "--private-key",
      params.privateKey,
      "--broadcast",
    ],
    {
      cwd: FOUNDRY_DIR,
      env: {
        DEPLOYER_PRIVATE_KEY: params.privateKey,
      },
    },
  );

  return parseReceiverAddress(`${result.stdout}\n${result.stderr}`);
}

async function getAddressFromPrivateKey(privateKey: string): Promise<string> {
  const result = await runCommand("cast", ["wallet", "address", "--private-key", privateKey]);
  return result.stdout.trim();
}

async function encodePublishCalldata(params: {
  runId: string;
  customerId: string;
  recommendedChain: string;
  score: number;
  timestamp: number;
}): Promise<string> {
  const result = await runCommand("cast", [
    "calldata",
    "publishRoute(string,string,string,uint256,uint256)",
    params.runId,
    params.customerId,
    params.recommendedChain,
    String(params.score),
    String(params.timestamp),
  ]);

  return result.stdout.trim();
}

async function publishRouteTx(params: {
  receiverAddress: string;
  rpcUrl: string;
  privateKey: string;
  runId: string;
  customerId: string;
  recommendedChain: string;
  score: number;
  timestamp: number;
}): Promise<PublishRouteTxReceipt> {
  const sendResult = await runCommand("cast", [
    "send",
    params.receiverAddress,
    "publishRoute(string,string,string,uint256,uint256)",
    params.runId,
    params.customerId,
    params.recommendedChain,
    String(params.score),
    String(params.timestamp),
    "--private-key",
    params.privateKey,
    "--rpc-url",
    params.rpcUrl,
    "--json",
  ]);

  const sendJson = JSON.parse(sendResult.stdout) as { transactionHash?: string };
  const txHash = sendJson.transactionHash;

  if (!txHash) {
    throw new Error("cast send output did not include transactionHash");
  }

  const receiptResult = await runCommand("cast", [
    "receipt",
    txHash,
    "--rpc-url",
    params.rpcUrl,
    "--json",
  ]);

  const receiptJson = JSON.parse(receiptResult.stdout) as {
    blockNumber?: unknown;
    logs?: Array<{ logIndex?: unknown }>;
  };

  const firstLog = receiptJson.logs?.[0];

  return {
    txHash,
    blockNumber: parseHexNumber(receiptJson.blockNumber),
    logs: [
      {
        eventName: "RoutePublished",
        logIndex: parseHexNumber(firstLog?.logIndex),
        args: {
          runId: params.runId,
          customerId: params.customerId,
          recommendedChain: params.recommendedChain,
          score: BigInt(params.score),
          timestamp: BigInt(params.timestamp),
        },
      },
    ],
  };
}

async function verifyPublished(params: {
  receiverAddress: string;
  rpcUrl: string;
  runId: string;
}): Promise<boolean> {
  const result = await runCommand("cast", [
    "call",
    params.receiverAddress,
    "isRunPublished(string)(bool)",
    params.runId,
    "--rpc-url",
    params.rpcUrl,
  ]);

  return result.stdout.trim().toLowerCase() === "true";
}

async function decodeLatestRoute(params: {
  receiverAddress: string;
  rpcUrl: string;
  customerId: string;
}): Promise<{ runId: string; recommendedChain: string; score: number; timestamp: number }> {
  const callResult = await runCommand("cast", [
    "call",
    params.receiverAddress,
    "getLatestRoute(string)(string,string,uint256,uint256)",
    params.customerId,
    "--rpc-url",
    params.rpcUrl,
  ]);

  const encoded = callResult.stdout.trim();

  const decoded = encoded.startsWith("0x")
    ? (
      await runCommand("cast", [
        "abi-decode",
        "(string,string,uint256,uint256)",
        encoded,
      ])
    ).stdout.trim()
    : encoded;
  const stringMatches = [...decoded.matchAll(/"([^"]*)"/g)].map((match) => match[1]);
  const withoutStrings = decoded.replace(/"[^"]*"/g, " ");
  const numericMatches = [...withoutStrings.matchAll(/\b\d+\b/g)].map((match) => Number(match[0]));

  return {
    runId: stringMatches[0] ?? "",
    recommendedChain: stringMatches[1] ?? "",
    score: numericMatches[0] ?? 0,
    timestamp: numericMatches[1] ?? 0,
  };
}

function startAnvil(rpcPort: number): ReturnType<typeof spawn> {
  return spawn("anvil", ["--port", String(rpcPort)], {
    cwd: REPO_ROOT,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

async function waitForRpc(rpcUrl: string): Promise<void> {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const result = await runCommand("cast", ["block-number", "--rpc-url", rpcUrl], {
      allowFailure: true,
    });
    if (result.code === 0) {
      return;
    }

    await new Promise((resolve) => setTimeout(resolve, 500));
  }

  throw new Error(`RPC ${rpcUrl} did not become ready in time`);
}

async function runUnauthorizedFailureProbe(params: {
  receiverAddress: string;
  rpcUrl: string;
  funderPrivateKey: string;
  payload: {
    runId: string;
    customerId: string;
    recommendedChain: string;
    score: number;
    timestamp: number;
  };
}): Promise<{ passed: boolean; error: string | null }> {
  const unauthorizedAddress = await getAddressFromPrivateKey(ANVIL_SECOND_PRIVATE_KEY);

  await runCommand("cast", [
    "send",
    unauthorizedAddress,
    "--value",
    "10000000000000000",
    "--private-key",
    params.funderPrivateKey,
    "--rpc-url",
    params.rpcUrl,
  ]);

  const result = await runCommand(
    "cast",
    [
      "send",
      params.receiverAddress,
      "publishRoute(string,string,string,uint256,uint256)",
      `${params.payload.runId}-unauthorized`,
      params.payload.customerId,
      params.payload.recommendedChain,
      String(params.payload.score),
      String(params.payload.timestamp),
      "--private-key",
      ANVIL_SECOND_PRIVATE_KEY,
      "--rpc-url",
      params.rpcUrl,
    ],
    { allowFailure: true },
  );

  if (result.code === 0) {
    return {
      passed: false,
      error: "Unauthorized publish unexpectedly succeeded",
    };
  }

  const combined = `${result.stdout}\n${result.stderr}`;
  const hasUnauthorizedMessage = /not authorized publisher/i.test(combined);

  return {
    passed: hasUnauthorizedMessage,
    error: hasUnauthorizedMessage ? null : combined.trim() || "Unauthorized publish failed unexpectedly",
  };
}

async function main(): Promise<void> {
  const network = parseNetwork();

  await assertCommandAvailable("cast");
  await assertCommandAvailable("forge");

  const rpcUrl =
    network === "local"
      ? process.env.LOCAL_RPC_URL ?? "http://127.0.0.1:8545"
      : process.env.BASE_SEPOLIA_RPC_URL;

  if (!rpcUrl) {
    throw new Error("BASE_SEPOLIA_RPC_URL is required for --network base-sepolia");
  }

  const deployerPrivateKey =
    network === "local"
      ? ANVIL_DEFAULT_PRIVATE_KEY
      : process.env.DEPLOYER_PRIVATE_KEY;

  if (!deployerPrivateKey) {
    throw new Error("DEPLOYER_PRIVATE_KEY is required");
  }

  let anvil: ReturnType<typeof spawn> | null = null;

  try {
    if (network === "local") {
      await assertCommandAvailable("anvil");
      anvil = startAnvil(8545);
      await waitForRpc(rpcUrl);
    }

    const receiverAddress =
      network === "base-sepolia" && process.env.RECEIVER_ADDRESS
        ? process.env.RECEIVER_ADDRESS
        : await deployReceiver({ rpcUrl, privateKey: deployerPrivateKey });

    const publisherAddress = await getAddressFromPrivateKey(deployerPrivateKey);

    const runId = `run-e2e-${Date.now()}`;
    const customerId = "customer-e2e";
    const allowedChains = ["base-sepolia", "arbitrum-sepolia"]; // Limited to 2 to respect the CLI simulation limit of 5 HTTP calls total (1 for defiLlama + 2 metrics * 2 chains = 5)

    process.stdout.write("\nSimulating cre-evaluator workflow execution via CLI...\n");
    const creCliResult = await runCommand(
      "bunx",
      [
        "cre",
        "workflow",
        "simulate",
        "cre-evaluator",
        "--target",
        "staging-settings",
        "--http-payload",
        JSON.stringify({
          requestId: runId,
          customerId,
          scenario: "normal",
          allowedChains,
        }),
        "--non-interactive",
        "--trigger-index",
        "0"
      ],
      {
        cwd: resolve(REPO_ROOT, "apps/cre-runtime"),
      }
    );

    const cliOutput = creCliResult.stdout;
    const resultMatch = cliOutput.match(/✓ Workflow Simulation Result:\n"({.*})"/);
    if (!resultMatch || !resultMatch[1]) {
      throw new Error(`Failed to parse Simulation Result from CLI stdout. Stdout:\n${cliOutput}`);
    }

    const simJsonStr = resultMatch[1].replace(/\\"/g, '"');
    const parsedSimulation = JSON.parse(simJsonStr);

    const scoreResult = {
      runId: parsedSimulation.runId,
      customerId: parsedSimulation.customerId,
      scenario: "normal" as const,
      recommendedChain: parsedSimulation.recommendedChain,
      confidence: parsedSimulation.confidence,
      ranked: parsedSimulation.ranked,
      reasonCodes: parsedSimulation.reasonCodes,
    };

    // Log the simulation result simply
    process.stdout.write(`Simulation Complete.\nRecommended Chain: ${scoreResult.recommendedChain}\nReason Codes: ${scoreResult.reasonCodes.join(", ")}\n`);

    const epochSeconds = Math.floor(Date.now() / 1000);
    const payload = toRouteUpdatedOnchain(scoreResult, epochSeconds);
    const activationDecision = classifyActivationByThreshold({
      threshold: 0.7,
      ranked: scoreResult.ranked,
    });
    const calldata = await encodePublishCalldata(payload);

    const onchainAdapter = createOnchainPublishAdapter({
      publishRoute: async (mappedPayload) =>
        publishRouteTx({
          receiverAddress,
          rpcUrl,
          privateKey: deployerPrivateKey,
          runId: mappedPayload.runId,
          customerId: mappedPayload.customerId,
          recommendedChain: mappedPayload.recommendedChain,
          score: mappedPayload.score,
          timestamp: mappedPayload.timestamp,
        }),
    });

    const publishResult = await onchainAdapter.publish(payload);

    if (publishResult.status !== "confirmed") {
      throw new Error(`Publish failed before confirmation: ${JSON.stringify(publishResult)}`);
    }

    const isRunPublished = await verifyPublished({
      receiverAddress,
      rpcUrl,
      runId: payload.runId,
    });

    const latestRoute = await decodeLatestRoute({
      receiverAddress,
      rpcUrl,
      customerId: payload.customerId,
    });

    const latestRouteMatches =
      latestRoute.runId === payload.runId &&
      latestRoute.recommendedChain === payload.recommendedChain &&
      latestRoute.score === payload.score &&
      latestRoute.timestamp === payload.timestamp;

    const unauthorizedProbe =
      network === "local"
        ? await runUnauthorizedFailureProbe({
          receiverAddress,
          rpcUrl,
          funderPrivateKey: deployerPrivateKey,
          payload,
        })
        : {
          passed: true,
          error: null,
        };

    const summary = {
      network,
      receiverAddress,
      publisherAddress,
      scoreResult,
      activationDecision,
      onchainPayload: payload,
      onchainPublication: publishResult,
      verification: {
        isRunPublished,
        latestRoute,
        latestRouteMatches,
      },
      failurePath: {
        unauthorizedPublisherRejected: unauthorizedProbe.passed,
        error: unauthorizedProbe.error,
      },
    };

    const artifactDir = resolve(REPO_ROOT, "apps/cre-workflows/artifacts");
    mkdirSync(artifactDir, { recursive: true });
    const artifactPath = resolve(artifactDir, "latest-summary.json");

    const runSummary = {
      runId: scoreResult.runId,
      customerId: scoreResult.customerId,
      status: publishResult.status === "confirmed" ? "ccipConfirmed" : "partialFailure",
      recommendedChain: scoreResult.recommendedChain,
      score: payload.score,
      confidence: scoreResult.confidence,
      thresholdUsed: activationDecision.thresholdUsed,
      activeChains: activationDecision.activeChains,
      inactiveChains: activationDecision.inactiveChains,
      reasonCodes: scoreResult.reasonCodes,
      timestamp: new Date(epochSeconds * 1000).toISOString(),
      scoreComponents: scoreResult.ranked[0]?.components ?? null,
      onchainPublication: {
        runId: scoreResult.runId,
        customerId: scoreResult.customerId,
        txHash: publishResult.txHash,
        blockNumber: publishResult.blockNumber,
        eventLogIndex: publishResult.eventLogIndex,
        publisherAddress,
        status: publishResult.status,
        errorCode: null,
        errorMessage: null,
        simulationId: publishResult.simulationId ?? null,
        simulationTraceUrl: publishResult.simulationTraceUrl ?? null,
        timestamp: new Date().toISOString(),
      },
      requestId: null,
      configVersion: null,
      scenario: "normal",
      attemptCount: 1,
      errorCode: null,
      errorMessage: null,
      receiverAddress,
      receiverVerification: {
        isRunPublished,
        onchainRoute: isRunPublished ? latestRoute : null,
        matchesPublished: latestRouteMatches,
        queriedAt: new Date().toISOString(),
      },
    };

    let existingRuns: any[] = [];
    try {
      if (require("fs").existsSync(artifactPath)) {
        const fileContent = require("fs").readFileSync(artifactPath, "utf-8");
        const parsed = JSON.parse(fileContent);
        if (Array.isArray(parsed.runs)) {
          existingRuns = parsed.runs;
        }
      }
    } catch (e) {
      // ignore parse errors
    }

    const updatedRuns = [runSummary, ...existingRuns].slice(0, 50);

    writeFileSync(artifactPath, JSON.stringify({ runs: updatedRuns }, null, 2));

    process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  } finally {
    if (anvil) {
      anvil.kill("SIGTERM");
    }
  }
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(1);
});
