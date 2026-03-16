import * as z from "zod";

// ── Onchain payload (maps 1:1 to RouteReceiver.sol storage) ────────

export const RouteUpdatedOnchainSchema = z.object({
    /** Canonical run identifier — correlation key across the full pipeline. */
    runId: z.string(),
    /** Customer this route update belongs to. */
    customerId: z.string(),
    /** Winning chain after scoring. */
    recommendedChain: z.string(),
    /** Scaled integer score (matches Solidity uint256, must be non-negative). */
    score: z.number().int().nonnegative(),
    /** Unix epoch seconds (matches Solidity uint256, must be non-negative). */
    timestamp: z.number().int().nonnegative(),
    /** Optional threshold represented in basis points for activation-state publishing. */
    thresholdBps: z.number().int().nonnegative().optional(),
    /** Optional active chain set used by threshold-based activation. */
    activeChains: z.array(z.string()).optional(),
    /** Optional inactive chain set used by threshold-based activation. */
    inactiveChains: z.array(z.string()).optional(),
});

export type RouteUpdatedOnchain = z.infer<typeof RouteUpdatedOnchainSchema>;

// ── Onchain publication lifecycle statuses ──────────────────────────

export const OnchainPublicationStatusSchema = z.enum([
    "pending",
    "confirmed",
    "failed",
    "duplicateRunId",
]);

export type OnchainPublicationStatus = z.infer<
    typeof OnchainPublicationStatusSchema
>;

export const ONCHAIN_PUBLICATION_STATUSES =
    OnchainPublicationStatusSchema.options;

// ── Onchain publication record (observability) ─────────────────────

export const OnchainPublicationRecordSchema = z.object({
    /** Links this publication to its parent scoring run. */
    runId: z.string(),
    /** Customer this publication belongs to. */
    customerId: z.string(),
    /** Transaction hash. Null until confirmed. */
    txHash: z.string().nullable(),
    /** Block number the tx was mined in. Null until confirmed. */
    blockNumber: z.number().int().nullable(),
    /** Log index of the RoutePublished event. Null until confirmed. */
    eventLogIndex: z.number().int().nullable(),
    /** Address that submitted the transaction. */
    publisherAddress: z.string().nullable(),
    /** Current publication lifecycle status. */
    status: OnchainPublicationStatusSchema,
    /** Error code when the publication fails. Null when not applicable. */
    errorCode: z.string().nullable(),
    /** Human-readable error description. Null when not applicable. */
    errorMessage: z.string().nullable(),
    /** Simulation identifier when pre-flight simulation ran. */
    simulationId: z.string().nullable(),
    /** Simulation trace URL when available. */
    simulationTraceUrl: z.string().url().nullable(),
    /** ISO-8601 timestamp of this record snapshot. */
    timestamp: z.string(),
});

export type OnchainPublicationRecord = z.infer<
    typeof OnchainPublicationRecordSchema
>;

// ── Publish result (adapter return type, discriminated union) ───────

const ConfirmedResultSchema = z.object({
    status: z.literal("confirmed"),
    txHash: z.string(),
    blockNumber: z.number().int(),
    eventLogIndex: z.number().int(),
    simulationId: z.string().optional(),
    simulationTraceUrl: z.string().url().optional(),
});

const DuplicateRunIdResultSchema = z.object({
    status: z.literal("duplicateRunId"),
    runId: z.string(),
});

const FailedResultSchema = z.object({
    status: z.literal("failed"),
    error: z.string(),
});

export const OnchainPublishResultSchema = z.discriminatedUnion("status", [
    ConfirmedResultSchema,
    DuplicateRunIdResultSchema,
    FailedResultSchema,
]);

export type OnchainPublishResult = z.infer<typeof OnchainPublishResultSchema>;
