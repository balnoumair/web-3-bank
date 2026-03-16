/**
 * Shared utility functions for CRE workflow runners.
 */

/**
 * Add milliseconds to an ISO-8601 timestamp and return the new ISO string.
 */
export function addMilliseconds(timestamp: string, milliseconds: number): string {
    return new Date(new Date(timestamp).getTime() + milliseconds).toISOString();
}

/**
 * Build a zero-padded sequential ID from a prefix and sequence number.
 * Example: buildSequenceId("run-dev-26", 3) → "run-dev-26-003"
 */
export function buildSequenceId(prefix: string, sequence: number): string {
    return `${prefix}-${String(sequence).padStart(3, "0")}`;
}
