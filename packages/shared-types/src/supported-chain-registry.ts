import type { CrePolicyErrorResponse } from "./intake/cre-policy-api-contract";

export interface SupportedChainRegistry {
  isSupportedChain(chain: string): boolean | Promise<boolean>;
}

export class InMemorySupportedChainRegistry implements SupportedChainRegistry {
  private readonly supportedChains: Set<string>;

  constructor(chains: readonly string[]) {
    this.supportedChains = new Set(chains);
  }

  isSupportedChain(chain: string): boolean {
    return this.supportedChains.has(chain);
  }
}

type ValidateAllowedChainsWithRegistryArgs = {
  requestId: string | null;
  allowedChains: readonly string[];
  registry: SupportedChainRegistry;
};

type ValidateAllowedChainsWithRegistrySuccess = {
  ok: true;
};

type ValidateAllowedChainsWithRegistryFailure = {
  ok: false;
  error: CrePolicyErrorResponse;
  unsupportedChains: string[];
};

export async function validateAllowedChainsWithRegistry(
  args: ValidateAllowedChainsWithRegistryArgs,
): Promise<
  | ValidateAllowedChainsWithRegistrySuccess
  | ValidateAllowedChainsWithRegistryFailure
> {
  const unsupportedChains: string[] = [];

  for (const chain of args.allowedChains) {
    const isSupported = await args.registry.isSupportedChain(chain);
    if (!isSupported) {
      unsupportedChains.push(chain);
    }
  }

  if (unsupportedChains.length > 0) {
    return {
      ok: false,
      error: {
        errorCode: "UNSUPPORTED_CHAIN",
        errorMessage: `unsupported chain '${unsupportedChains[0]}'`,
        requestId: args.requestId,
      },
      unsupportedChains,
    };
  }

  return {
    ok: true,
  };
}
