import * as z from "zod";

import { ActivationThresholdSchema, ScenarioNameSchema } from "../core-types";

export const CrePolicyErrorCodeSchema = z.enum([
  "UNAUTHORIZED",
  "INVALID_PAYLOAD",
  "UNSUPPORTED_CHAIN",
  "INVALID_SCENARIO",
  "CUSTOMER_NOT_FOUND",
  "STORE_UNAVAILABLE",
]);

export type CrePolicyErrorCode = z.infer<typeof CrePolicyErrorCodeSchema>;

export const CRE_POLICY_ERROR_CODES = CrePolicyErrorCodeSchema.options;

export const CrePolicyErrorResponseSchema = z.object({
  errorCode: CrePolicyErrorCodeSchema,
  errorMessage: z.string(),
  requestId: z.string().nullable(),
});

export type CrePolicyErrorResponse = z.infer<typeof CrePolicyErrorResponseSchema>;

const NonEmptyUniqueChainsSchema = z
  .array(z.string())
  .min(1)
  .superRefine((chains, context) => {
    if (new Set(chains).size !== chains.length) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        message: "allowedChains must be unique",
      });
    }
  });

export const CrePolicyConfigUpsertRequestSchema = z.object({
  requestId: z.string(),
  customerId: z.string(),
  allowedChains: NonEmptyUniqueChainsSchema,
  active: z.boolean(),
  activationThreshold: ActivationThresholdSchema,
});

export type CrePolicyConfigUpsertRequest = z.infer<typeof CrePolicyConfigUpsertRequestSchema>;

export const CrePolicyConfigUpsertResponseSchema = z.object({
  requestId: z.string(),
  customerId: z.string(),
  configVersion: z.number().int().nonnegative(),
  updatedAt: z.string(),
});

export type CrePolicyConfigUpsertResponse = z.infer<typeof CrePolicyConfigUpsertResponseSchema>;

export const CrePolicyScenarioUpdateRequestSchema = z.object({
  requestId: z.string(),
  customerId: z.string(),
  scenario: ScenarioNameSchema,
});

export type CrePolicyScenarioUpdateRequest = z.infer<typeof CrePolicyScenarioUpdateRequestSchema>;

export const CrePolicyScenarioUpdateResponseSchema = z.object({
  requestId: z.string(),
  customerId: z.string(),
  scenario: ScenarioNameSchema,
  configVersion: z.number().int().nonnegative(),
  effectiveAt: z.string(),
  appliesToRun: z.literal("next"),
});

export type CrePolicyScenarioUpdateResponse = z.infer<
  typeof CrePolicyScenarioUpdateResponseSchema
>;

export const CrePolicyGetConfigParamsSchema = z.object({
  customerId: z.string(),
});

export type CrePolicyGetConfigParams = z.infer<typeof CrePolicyGetConfigParamsSchema>;

export const CrePolicyConfigSnapshotSchema = z.object({
  customerId: z.string(),
  configVersion: z.number().int().nonnegative(),
  updatedAt: z.string(),
  allowedChains: NonEmptyUniqueChainsSchema,
  active: z.boolean(),
  scenario: ScenarioNameSchema,
  activationThreshold: ActivationThresholdSchema,
});

export type CrePolicyConfigSnapshot = z.infer<typeof CrePolicyConfigSnapshotSchema>;

export const CrePolicyGetConfigResponseSchema = CrePolicyConfigSnapshotSchema;

export type CrePolicyGetConfigResponse = z.infer<typeof CrePolicyGetConfigResponseSchema>;

type ParseSuccess<T> = {
  ok: true;
  data: T;
};

type ParseFailure = {
  ok: false;
  error: CrePolicyErrorResponse;
};

function buildInvalidPayloadError(
  errorMessage: string,
  requestId?: string | null,
): CrePolicyErrorResponse {
  return {
    errorCode: "INVALID_PAYLOAD",
    errorMessage,
    requestId: requestId ?? null,
  };
}

export function parseCrePolicyConfigUpsertRequest(
  payload: unknown,
): ParseSuccess<CrePolicyConfigUpsertRequest> | ParseFailure {
  const parsed = CrePolicyConfigUpsertRequestSchema.safeParse(payload);
  if (parsed.success) {
    return {
      ok: true,
      data: parsed.data,
    };
  }

  const requestId =
    typeof payload === "object" && payload !== null && "requestId" in payload
      ? (payload as Record<string, unknown>).requestId
      : null;

  return {
    ok: false,
    error: buildInvalidPayloadError(
      "invalid POST /config payload",
      typeof requestId === "string" ? requestId : null,
    ),
  };
}

export function parseCrePolicyScenarioUpdateRequest(
  payload: unknown,
): ParseSuccess<CrePolicyScenarioUpdateRequest> | ParseFailure {
  const parsed = CrePolicyScenarioUpdateRequestSchema.safeParse(payload);
  if (parsed.success) {
    return {
      ok: true,
      data: parsed.data,
    };
  }

  const requestId =
    typeof payload === "object" && payload !== null && "requestId" in payload
      ? (payload as Record<string, unknown>).requestId
      : null;

  return {
    ok: false,
    error: buildInvalidPayloadError(
      "invalid POST /scenario payload",
      typeof requestId === "string" ? requestId : null,
    ),
  };
}
