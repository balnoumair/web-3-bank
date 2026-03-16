import type { OnchainPublishResult, RouteUpdatedOnchain } from "./onchain-evidence-schema";

// ── Onchain publish port (dependency injection boundary) ───────────

/**
 * Port interface for publishing route scoring results onchain.
 * Implementations handle provider/signer details; callers only
 * deal with typed payloads and results.
 */
export interface OnchainPublishPort {
    publish(payload: RouteUpdatedOnchain): Promise<OnchainPublishResult>;
}
