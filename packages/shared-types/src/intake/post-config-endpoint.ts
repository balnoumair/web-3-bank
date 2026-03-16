import type {
  ConfigStorePort,
  ConfigStorePortSnapshot,
  ConfigStorePortUpsertConfigInput,
} from "../config-store-port";
import {
  type CrePolicyConfigUpsertResponse,
  type CrePolicyErrorResponse,
  parseCrePolicyConfigUpsertRequest,
} from "./cre-policy-api-contract";
import {
  type CrePolicyErrorHttpStatus,
  isStoreUnavailableError,
  toCrePolicyErrorResponse,
} from "./intake-error-mapping";
import type { SupportedChainRegistry } from "../supported-chain-registry";
import { validateAllowedChainsWithRegistry } from "../supported-chain-registry";

type PostConfigSuccessResponse = {
  status: 200;
  body: CrePolicyConfigUpsertResponse;
};

type PostConfigErrorResponse = {
  status: CrePolicyErrorHttpStatus;
  body: CrePolicyErrorResponse;
};

export type PostConfigEndpointResponse =
  | PostConfigSuccessResponse
  | PostConfigErrorResponse;

type PostConfigEndpointRequest = {
  authorizationHeader: string | undefined;
  payload: unknown;
  now?: string;
};

type PostConfigEndpointDeps = {
  store: ConfigStorePort;
  registry: SupportedChainRegistry;
  authorize: (authorizationHeader: string | undefined) => Promise<boolean> | boolean;
};

function extractRequestId(payload: unknown): string | null {
  if (typeof payload === "object" && payload !== null && "requestId" in payload) {
    const requestId = (payload as Record<string, unknown>).requestId;
    return typeof requestId === "string" ? requestId : null;
  }

  return null;
}

function toUpsertResponse(args: {
  requestId: string;
  snapshot: ConfigStorePortSnapshot;
}): CrePolicyConfigUpsertResponse {
  return {
    requestId: args.requestId,
    customerId: args.snapshot.customerId,
    configVersion: args.snapshot.configVersion,
    updatedAt: args.snapshot.updatedAt,
  };
}

function extractCustomerId(payload: unknown): string | null {
  if (typeof payload === "object" && payload !== null && "customerId" in payload) {
    const customerId = (payload as Record<string, unknown>).customerId;
    return typeof customerId === "string" ? customerId : null;
  }

  return null;
}

export function createPostConfigEndpoint(deps: PostConfigEndpointDeps): {
  (request: PostConfigEndpointRequest): Promise<PostConfigEndpointResponse>;
} {
  return async (request) => {
    const requestId = extractRequestId(request.payload);
    const customerId = extractCustomerId(request.payload);
    const timestamp = request.now ?? new Date().toISOString();

    const appendAudit = async (args: {
      requestId: string;
      customerId: string;
      status: "accepted" | "rejected";
      configVersion: number | null;
      errorCode: string | null;
      errorMessage: string | null;
    }): Promise<void> => {
      await deps.store.appendAuditRecord({
        requestId: args.requestId,
        customerId: args.customerId,
        action: "config.upsert",
        timestamp,
        status: args.status,
        configVersion: args.configVersion,
        errorCode: args.errorCode,
        errorMessage: args.errorMessage,
      });
    };

    const authorized = await deps.authorize(request.authorizationHeader);
    if (!authorized) {
      await appendAudit({
        requestId: requestId ?? "unknown-request",
        customerId: customerId ?? "unknown-customer",
        status: "rejected",
        configVersion: null,
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
      });

      return toCrePolicyErrorResponse({
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId,
      });
    }

    const parsedPayload = parseCrePolicyConfigUpsertRequest(request.payload);
    if (!parsedPayload.ok) {
      await appendAudit({
        requestId: requestId ?? "unknown-request",
        customerId: customerId ?? "unknown-customer",
        status: "rejected",
        configVersion: null,
        errorCode: parsedPayload.error.errorCode,
        errorMessage: parsedPayload.error.errorMessage,
      });

      return {
        status: 400,
        body: parsedPayload.error,
      };
    }

    const supportedValidation = await validateAllowedChainsWithRegistry({
      requestId: parsedPayload.data.requestId,
      allowedChains: parsedPayload.data.allowedChains,
      registry: deps.registry,
    });

    if (!supportedValidation.ok) {
      await appendAudit({
        requestId: parsedPayload.data.requestId,
        customerId: parsedPayload.data.customerId,
        status: "rejected",
        configVersion: null,
        errorCode: supportedValidation.error.errorCode,
        errorMessage: supportedValidation.error.errorMessage,
      });

      return {
        status: 400,
        body: supportedValidation.error,
      };
    }

    const upsertInput: ConfigStorePortUpsertConfigInput = {
      ...parsedPayload.data,
      timestamp,
    };

    try {
      const result = await deps.store.upsertConfig(upsertInput);

      await appendAudit({
        requestId: parsedPayload.data.requestId,
        customerId: parsedPayload.data.customerId,
        status: "accepted",
        configVersion: result.snapshot.configVersion,
        errorCode: null,
        errorMessage: null,
      });

      return {
        status: 200,
        body: toUpsertResponse({
          requestId: parsedPayload.data.requestId,
          snapshot: result.snapshot,
        }),
      };
    } catch (error) {
      if (isStoreUnavailableError(error)) {
        try {
          await appendAudit({
            requestId: parsedPayload.data.requestId,
            customerId: parsedPayload.data.customerId,
            status: "rejected",
            configVersion: null,
            errorCode: "STORE_UNAVAILABLE",
            errorMessage: "config store unavailable",
          });
        } catch {
          // ignore audit write failures when storage is unavailable
        }

        return toCrePolicyErrorResponse({
          errorCode: "STORE_UNAVAILABLE",
          errorMessage: "config store unavailable",
          requestId: parsedPayload.data.requestId,
        });
      }

      throw error;
    }
  };
}
