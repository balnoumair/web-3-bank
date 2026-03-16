import {
  HTTPCapability,
  Runner,
  decodeJson,
  handler,
  type HTTPPayload,
  type Runtime,
} from "@chainlink/cre-sdk";

// ── Config type for this workflow (loaded from config.staging.json) ──

type PolicyConfig = {
  workflowName: string;
  defaultActivationThreshold: number;
};

// ── Request / Response shapes ───────────────────────────────────────

type PolicyUpdateRequest = {
  requestId: string;
  customerId: string;
  allowedChains: string[];
  active: boolean;
  activationThreshold?: number;
};

type PolicyUpdateResponse = {
  requestId: string;
  customerId: string;
  configVersion: number;
  allowedChains: string[];
  active: boolean;
  activationThreshold: number;
  updatedAt: string;
};

type PolicyErrorResponse = {
  errorCode: string;
  errorMessage: string;
  requestId: string | null;
};

// ── Validation helpers ──────────────────────────────────────────────

function isValidRequest(payload: unknown): payload is PolicyUpdateRequest {
  if (typeof payload !== "object" || payload === null) return false;
  const obj = payload as Record<string, unknown>;
  return (
    typeof obj.requestId === "string" &&
    typeof obj.customerId === "string" &&
    Array.isArray(obj.allowedChains) &&
    obj.allowedChains.length > 0 &&
    obj.allowedChains.every((c: unknown) => typeof c === "string") &&
    typeof obj.active === "boolean"
  );
}

// ── Simple in-memory version counter (per-customer) ─────────────────

const customerVersions = new Map<string, number>();

function nextVersion(customerId: string): number {
  const current = customerVersions.get(customerId) ?? 0;
  const next = current + 1;
  customerVersions.set(customerId, next);
  return next;
}

// ── Handler ─────────────────────────────────────────────────────────

const onPolicyUpdate = (
  runtime: Runtime<PolicyConfig>,
  payload: HTTPPayload,
): string => {
  const raw = decodeJson(payload.input) as Record<string, unknown>;
  runtime.log(`CRE Policy policy request received: ${JSON.stringify(raw)}`);

  // Validate the inbound payload
  if (!isValidRequest(raw)) {
    const errorResponse: PolicyErrorResponse = {
      errorCode: "INVALID_PAYLOAD",
      errorMessage:
        "Missing or invalid fields: requestId (string), customerId (string), allowedChains (string[], min 1), active (boolean) are required",
      requestId: typeof raw.requestId === "string" ? raw.requestId : null,
    };
    runtime.log(`CRE Policy validation failed: ${errorResponse.errorMessage}`);
    return JSON.stringify(errorResponse);
  }

  // Resolve activation threshold: request value > config default > 0.7
  const threshold =
    typeof raw.activationThreshold === "number" &&
    raw.activationThreshold >= 0 &&
    raw.activationThreshold <= 1
      ? raw.activationThreshold
      : runtime.config.defaultActivationThreshold;

  // Build the accepted config snapshot
  const configVersion = nextVersion(raw.customerId);
  const response: PolicyUpdateResponse = {
    requestId: raw.requestId,
    customerId: raw.customerId,
    configVersion,
    allowedChains: raw.allowedChains,
    active: raw.active,
    activationThreshold: threshold,
    updatedAt: new Date().toISOString(),
  };

  runtime.log(
    `CRE Policy policy accepted: customer=${raw.customerId} version=${configVersion} chains=[${raw.allowedChains.join(",")}] threshold=${threshold}`,
  );

  return JSON.stringify(response);
};

// ── Workflow initialisation ─────────────────────────────────────────

const initWorkflow = () => {
  const http = new HTTPCapability();
  return [handler(http.trigger({}), onPolicyUpdate)];
};

export async function main() {
  const runner = await Runner.newRunner<PolicyConfig>();
  await runner.run(initWorkflow);
}
