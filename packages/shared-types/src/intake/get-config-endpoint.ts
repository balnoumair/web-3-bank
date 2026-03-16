import type { ConfigStorePort } from "../config-store-port";
import type {
  CrePolicyConfigSnapshot,
  CrePolicyErrorResponse,
  CrePolicyGetConfigParams,
} from "./cre-policy-api-contract";
import { CrePolicyGetConfigParamsSchema } from "./cre-policy-api-contract";
import { isStoreUnavailableError, toCrePolicyErrorResponse } from "./intake-error-mapping";

type GetConfigSuccessResponse = {
  status: 200;
  body: CrePolicyConfigSnapshot;
};

type GetConfigErrorResponse = {
  status: 400 | 401 | 404 | 503;
  body: CrePolicyErrorResponse;
};

export type GetConfigEndpointResponse =
  | GetConfigSuccessResponse
  | GetConfigErrorResponse;

type GetConfigEndpointRequest = {
  authorizationHeader: string | undefined;
  params: unknown;
};

type GetConfigEndpointDeps = {
  store: ConfigStorePort;
  authorize: (authorizationHeader: string | undefined) => Promise<boolean> | boolean;
};

export function createGetConfigEndpoint(deps: GetConfigEndpointDeps): {
  (request: GetConfigEndpointRequest): Promise<GetConfigEndpointResponse>;
} {
  return async (request) => {
    const authorized = await deps.authorize(request.authorizationHeader);
    if (!authorized) {
      return toCrePolicyErrorResponse({
        errorCode: "UNAUTHORIZED",
        errorMessage: "unauthorized request",
        requestId: null,
      });
    }

    const parsedParams = CrePolicyGetConfigParamsSchema.safeParse(request.params);
    if (!parsedParams.success) {
      return toCrePolicyErrorResponse({
        errorCode: "INVALID_PAYLOAD",
        errorMessage: "invalid GET /config/:customerId params",
        requestId: null,
      });
    }

    const params: CrePolicyGetConfigParams = parsedParams.data;

    try {
      const snapshot = await deps.store.getConfigSnapshot(params.customerId);
      if (!snapshot) {
        return toCrePolicyErrorResponse({
          errorCode: "CUSTOMER_NOT_FOUND",
          errorMessage: "customer not found",
          requestId: null,
        });
      }

      return {
        status: 200,
        body: snapshot,
      };
    } catch (error) {
      if (isStoreUnavailableError(error)) {
        return toCrePolicyErrorResponse({
          errorCode: "STORE_UNAVAILABLE",
          errorMessage: "config store unavailable",
          requestId: null,
        });
      }

      throw error;
    }
  };
}
