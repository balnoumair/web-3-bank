import { describe, expect, it } from "vitest";

import {
  InMemorySupportedChainRegistry,
  validateAllowedChainsWithRegistry,
} from "../src";
import { SUPPORTED_CHAIN_REGISTRY_CASES } from "./fixtures/supported-chain-registry-cases";

describe("SupportedChainRegistry", () => {
  it("accepts allowedChains when every chain is supported", async () => {
    const registry = new InMemorySupportedChainRegistry(
      SUPPORTED_CHAIN_REGISTRY_CASES.supportedChains,
    );

    const validation = await validateAllowedChainsWithRegistry({
      requestId: "req-33-supported",
      allowedChains: SUPPORTED_CHAIN_REGISTRY_CASES.allSupportedAllowedChains,
      registry,
    });

    expect(validation).toEqual({
      ok: true,
    });
  });

  it("rejects unknown chains as UNSUPPORTED_CHAIN", async () => {
    const registry = new InMemorySupportedChainRegistry(
      SUPPORTED_CHAIN_REGISTRY_CASES.supportedChains,
    );

    const validation = await validateAllowedChainsWithRegistry({
      requestId: "req-33-unsupported",
      allowedChains: SUPPORTED_CHAIN_REGISTRY_CASES.containsUnsupportedAllowedChains,
      registry,
    });

    expect(validation).toEqual({
      ok: false,
      error: {
        errorCode: "UNSUPPORTED_CHAIN",
        errorMessage: "unsupported chain 'polygon-amoy'",
        requestId: "req-33-unsupported",
      },
      unsupportedChains: ["polygon-amoy"],
    });
  });

  it("uses adapter-injected registry behavior instead of hardcoded checks", async () => {
    const calls: string[] = [];
    const registry = {
      isSupportedChain: async (chain: string) => {
        calls.push(chain);
        return chain !== "polygon-amoy";
      },
    };

    const validation = await validateAllowedChainsWithRegistry({
      requestId: "req-33-adapter",
      allowedChains: SUPPORTED_CHAIN_REGISTRY_CASES.containsUnsupportedAllowedChains,
      registry,
    });

    expect(validation.ok).toBe(false);
    expect(calls).toEqual(SUPPORTED_CHAIN_REGISTRY_CASES.containsUnsupportedAllowedChains);
  });
});
