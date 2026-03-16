import type {
  ConfigStorePort,
  ConfigStorePortUpdateScenarioInput,
} from "../config-store-port";
import {
  type CrePolicyErrorResponse,
  type CrePolicyScenarioUpdateResponse,
  CrePolicyScenarioUpdateRequestSchema,
} from "./cre-policy-api-contract";
import { ScenarioNameSchema } from "../core-types";
import { isStoreUnavailableError, toCrePolicyErrorResponse } from "./intake-error-mapping";

type PostScenarioSuccessResponse = {
  status: 200;
  body: CrePolicyScenarioUpdateResponse;
};

type PostScenarioErrorResponse = {
  status: 400 | 401 | 404 | 503;
  body: CrePolicyErrorResponse;
};

export type PostScenarioEndpointResponse =
  | PostScenarioSuccessResponse
  | PostScenarioErrorResponse;

type PostScenarioEndpointRequest = {
  authorizationHeader: string | undefined;
  payload: unknown;
  now?: string;
};

type PostScenarioEndpointDeps = {
  store: ConfigStorePort;
  authorize: (authorizationHeader: string | undefined) => Promise<boolean> | boolean;
};

function extractRequestId(payload: unknown): string | null {
  if (typeof payload === "object" && payload !== null && "requestId" in payload) {
    const requestId = (payload as Record<string, unknown>).requestId;
    return typeof requestId === "string" ? requestId : null;
  }

  return null;
}

function isInvalidScenarioPayload(payload: unknown): boolean {
  if (typeof payload !== "object" || payload === null || !("scenario" in payload)) {
    return false;
  }

  const scenario = (payload as Record<string, unknown>).scenario;
  return !ScenarioNameSchema.safeParse(scenario).success;
}

function extractCustomerId(payload: unknown): string | null {
  if (typeof payload === "object" && payload !== null && "customerId" in payload) {
    const customerId = (payload as Record<string, unknown>).customerId;
    return typeof customerId === "string" ? customerId : null;
  }

  return null;
}

export function createPostScenarioEndpoint(deps: PostScenarioEndpointDeps): {
  (request: PostScenarioEndpointRequest): Promise<PostScenarioEndpointResponse>;
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
        action: "scenario.update",
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

    if (isInvalidScenarioPayload(request.payload)) {
      await appendAudit({
        requestId: requestId ?? "unknown-request",
        customerId: customerId ?? "unknown-customer",
        status: "rejected",
        configVersion: null,
        errorCode: "INVALID_SCENARIO",
        errorMessage: "invalid scenario value",
      });

      return toCrePolicyErrorResponse({
        errorCode: "INVALID_SCENARIO",
        errorMessage: "invalid scenario value",
        requestId,
      });
    }

    const parsedPayload = CrePolicyScenarioUpdateRequestSchema.safeParse(request.payload);
    if (!parsedPayload.success) {
      await appendAudit({
        requestId: requestId ?? "unknown-request",
        customerId: customerId ?? "unknown-customer",
        status: "rejected",
        configVersion: null,
        errorCode: "INVALID_PAYLOAD",
        errorMessage: "invalid POST /scenario payload",
      });

      return toCrePolicyErrorResponse({
        errorCode: "INVALID_PAYLOAD",
        errorMessage: "invalid POST /scenario payload",
        requestId,
      });
    }

    const updateInput: ConfigStorePortUpdateScenarioInput = {
      ...parsedPayload.data,
      timestamp,
    };

    try {
      const result = await deps.store.updateScenario(updateInput);
      if (!result.snapshot) {
        await appendAudit({
          requestId: parsedPayload.data.requestId,
          customerId: parsedPayload.data.customerId,
          status: "rejected",
          configVersion: null,
          errorCode: "CUSTOMER_NOT_FOUND",
          errorMessage: "customer not found",
        });

        return toCrePolicyErrorResponse({
          errorCode: "CUSTOMER_NOT_FOUND",
          errorMessage: "customer not found",
          requestId: parsedPayload.data.requestId,
        });
      }

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
        body: {
          requestId: parsedPayload.data.requestId,
          customerId: parsedPayload.data.customerId,
          scenario: result.snapshot.scenario,
          configVersion: result.snapshot.configVersion,
          effectiveAt: result.effectiveAt ?? updateInput.timestamp,
          appliesToRun: result.appliesToRun,
        },
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
