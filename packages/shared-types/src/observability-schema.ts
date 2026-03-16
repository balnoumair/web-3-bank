import * as z from "zod";
import { RunStatusSchema, ScenarioNameSchema } from "./core-types";
import { ScoringV1ReasonCodeSchema } from "./scoring-schema";

// ── CCIP delivery lifecycle statuses ────────────────────────────────

export const CcipDeliveryStatusSchema = z.enum([
    "pending",
    "sent",
    "confirmed",
    "failed",
]);

export type CcipDeliveryStatus = z.infer<typeof CcipDeliveryStatusSchema>;

export const CCIP_DELIVERY_STATUSES = CcipDeliveryStatusSchema.options;

// ── Scoring run record ──────────────────────────────────────────────

export const ScoringRunRecordSchema = z.object({
    /** Unique run identifier — idempotency key across retries. */
    runId: z.string(),
    /** Customer this run belongs to. */
    customerId: z.string(),
    /** Intake request that produced the consumed snapshot. */
    requestId: z.string().nullable(),
    /** Snapshot version pinned at run start. */
    configVersion: z.number().int().nonnegative().nullable(),
    /** Snapshot updatedAt pinned at run start. */
    snapshotUpdatedAt: z.string().nullable(),
    /** Current run state (from the RunStatus union). */
    status: RunStatusSchema,
    /** ISO-8601 timestamp of this record snapshot. */
    timestamp: z.string(),
    /** Scenario used for scoring fallbacks. */
    scenario: ScenarioNameSchema,
    /** Recommended chain after scoring. Null before `scored` state. */
    recommendedChain: z.string().nullable(),
    /** Top score. Null before `scored` state. */
    score: z.number().nullable(),
    /** Scoring reason codes emitted during the run. */
    reasonCodes: z.array(ScoringV1ReasonCodeSchema),
    /** Number of CCIP send attempts made during this run. */
    attemptCount: z.number(),
    /** Error code when the run ends in a failure state. */
    errorCode: z.string().nullable(),
    /** Human-readable error description. */
    errorMessage: z.string().nullable(),
});

export type ScoringRunRecord = z.infer<typeof ScoringRunRecordSchema>;

// ── CCIP delivery record ────────────────────────────────────────────

export const CcipDeliveryRecordSchema = z.object({
    /** Unique delivery identifier. */
    deliveryId: z.string(),
    /** Links this delivery to its parent scoring run. */
    runId: z.string(),
    /** CCIP message ID returned by the router. Null if send has not succeeded. */
    ccipMessageId: z.string().nullable(),
    /** Source chain for the CCIP message. */
    sourceChain: z.string(),
    /** Destination chain for the CCIP message. */
    destinationChain: z.string(),
    /** Current delivery lifecycle status. */
    status: CcipDeliveryStatusSchema,
    /** Number of send attempts for this delivery. */
    attemptCount: z.number(),
    /** Error code when the delivery fails. */
    errorCode: z.string().nullable(),
    /** Human-readable error description. */
    errorMessage: z.string().nullable(),
    /** ISO-8601 timestamp of this record snapshot. */
    timestamp: z.string(),
});

export type CcipDeliveryRecord = z.infer<typeof CcipDeliveryRecordSchema>;
