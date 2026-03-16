import { describe, expect, it } from "vitest";

import type {
    OnchainPublicationRecord,
    OnchainPublishResult,
    RouteUpdatedOnchain,
} from "../src";

import {
    OnchainPublicationRecordSchema,
    OnchainPublicationStatusSchema,
    OnchainPublishResultSchema,
    ONCHAIN_PUBLICATION_STATUSES,
    RouteUpdatedOnchainSchema,
} from "../src";

import {
    ALL_EXPECTED_PUBLICATION_STATUSES,
    ONCHAIN_PAYLOAD_CASES,
    ONCHAIN_PUBLICATION_CASES,
    ONCHAIN_PUBLISH_RESULT_CASES,
} from "./fixtures/onchain-evidence-cases";

describe("onchain evidence schema", () => {
    describe("RouteUpdatedOnchain", () => {
        it("validates a valid onchain payload", () => {
            const payload: RouteUpdatedOnchain = ONCHAIN_PAYLOAD_CASES.valid;
            expect(() => RouteUpdatedOnchainSchema.parse(payload)).not.toThrow();
        });

        it("validates minimal onchain payload (zero values)", () => {
            const payload: RouteUpdatedOnchain = ONCHAIN_PAYLOAD_CASES.minimal;
            expect(() => RouteUpdatedOnchainSchema.parse(payload)).not.toThrow();
            expect(payload.score).toBe(0);
            expect(payload.timestamp).toBe(0);
        });

        it("validates activation payload extension fields", () => {
            const payload: RouteUpdatedOnchain = ONCHAIN_PAYLOAD_CASES.activation;
            expect(() => RouteUpdatedOnchainSchema.parse(payload)).not.toThrow();
            expect(payload.thresholdBps).toBe(7000);
            expect(payload.activeChains).toEqual(["base-sepolia", "arbitrum-sepolia"]);
            expect(payload.inactiveChains).toEqual(["optimism-sepolia"]);
        });

        it("uses integer timestamp (not ISO string)", () => {
            const payload = ONCHAIN_PAYLOAD_CASES.valid;
            expect(typeof payload.timestamp).toBe("number");
            expect(Number.isInteger(payload.timestamp)).toBe(true);
        });

        it("uses integer score (scaled, not float)", () => {
            const payload = ONCHAIN_PAYLOAD_CASES.valid;
            expect(typeof payload.score).toBe("number");
            expect(Number.isInteger(payload.score)).toBe(true);
        });

        it("rejects missing required fields", () => {
            const partial = { runId: "run-1", customerId: "cust-1" };
            expect(() => RouteUpdatedOnchainSchema.parse(partial)).toThrow();
        });

        it("rejects non-integer timestamp", () => {
            const invalid = { ...ONCHAIN_PAYLOAD_CASES.valid, timestamp: 17.5 };
            expect(() => RouteUpdatedOnchainSchema.parse(invalid)).toThrow();
        });

        it("rejects non-integer score", () => {
            const invalid = { ...ONCHAIN_PAYLOAD_CASES.valid, score: 0.82 };
            expect(() => RouteUpdatedOnchainSchema.parse(invalid)).toThrow();
        });

        it("rejects negative score", () => {
            const invalid = { ...ONCHAIN_PAYLOAD_CASES.valid, score: -1 };
            expect(() => RouteUpdatedOnchainSchema.parse(invalid)).toThrow();
        });

        it("rejects negative timestamp", () => {
            const invalid = { ...ONCHAIN_PAYLOAD_CASES.valid, timestamp: -100 };
            expect(() => RouteUpdatedOnchainSchema.parse(invalid)).toThrow();
        });
    });

    describe("OnchainPublicationStatus", () => {
        it("has exactly the expected publication statuses", () => {
            expect([...ONCHAIN_PUBLICATION_STATUSES]).toEqual(
                [...ALL_EXPECTED_PUBLICATION_STATUSES],
            );
        });

        it("has no duplicate statuses", () => {
            const unique = new Set(ONCHAIN_PUBLICATION_STATUSES);
            expect(unique.size).toBe(ONCHAIN_PUBLICATION_STATUSES.length);
        });
    });

    describe("OnchainPublicationRecord", () => {
        it("validates a confirmed publication record", () => {
            const record: OnchainPublicationRecord =
                ONCHAIN_PUBLICATION_CASES.confirmed;
            expect(() =>
                OnchainPublicationRecordSchema.parse(record),
            ).not.toThrow();
        });

        it("includes tx evidence on confirmed publish", () => {
            const record = ONCHAIN_PUBLICATION_CASES.confirmed;
            expect(record.txHash).toBeDefined();
            expect(record.blockNumber).toBeDefined();
            expect(record.eventLogIndex).toBeDefined();
            expect(record.status).toBe("confirmed");
        });

        it("has null tx evidence on failed publish", () => {
            const record: OnchainPublicationRecord =
                ONCHAIN_PUBLICATION_CASES.failed;
            expect(() =>
                OnchainPublicationRecordSchema.parse(record),
            ).not.toThrow();
            expect(record.txHash).toBeNull();
            expect(record.blockNumber).toBeNull();
            expect(record.eventLogIndex).toBeNull();
            expect(record.errorCode).toBe("PUBLISH_REVERTED");
        });

        it("validates a duplicate runId publication record", () => {
            const record: OnchainPublicationRecord =
                ONCHAIN_PUBLICATION_CASES.duplicate;
            expect(() =>
                OnchainPublicationRecordSchema.parse(record),
            ).not.toThrow();
            expect(record.status).toBe("duplicateRunId");
        });

        it("validates a pending publication record", () => {
            const record: OnchainPublicationRecord =
                ONCHAIN_PUBLICATION_CASES.pending;
            expect(() =>
                OnchainPublicationRecordSchema.parse(record),
            ).not.toThrow();
            expect(record.status).toBe("pending");
        });

        it("validates publication record when simulation evidence is present", () => {
            const record: OnchainPublicationRecord =
                ONCHAIN_PUBLICATION_CASES.confirmedWithSimulation;
            expect(() =>
                OnchainPublicationRecordSchema.parse(record),
            ).not.toThrow();
            expect(record.simulationId).toBe("sim-1234567890abcdef");
            expect(record.simulationTraceUrl).toContain("dashboard.tenderly.co");
        });

        it("accepts null simulation fields for graceful degradation", () => {
            const record: OnchainPublicationRecord =
                ONCHAIN_PUBLICATION_CASES.confirmed;
            expect(() =>
                OnchainPublicationRecordSchema.parse(record),
            ).not.toThrow();
            expect(record.simulationId).toBeNull();
            expect(record.simulationTraceUrl).toBeNull();
        });

        it("rejects malformed simulationTraceUrl", () => {
            const invalid = {
                ...ONCHAIN_PUBLICATION_CASES.confirmedWithSimulation,
                simulationTraceUrl: "not-a-url",
            };
            expect(() =>
                OnchainPublicationRecordSchema.parse(invalid),
            ).toThrow();
        });

        it("rejects invalid publication status", () => {
            const invalid = {
                ...ONCHAIN_PUBLICATION_CASES.confirmed,
                status: "invalid-status",
            };
            expect(() =>
                OnchainPublicationRecordSchema.parse(invalid),
            ).toThrow();
        });

        it("links publication to run via runId", () => {
            const record = ONCHAIN_PUBLICATION_CASES.confirmed;
            expect(record.runId).toBe("run-onchain-001");
            expect(record.customerId).toBe("customer-1");
        });
    });

    describe("OnchainPublishResult (adapter return type)", () => {
        it("validates a confirmed result with tx evidence", () => {
            const result: OnchainPublishResult =
                ONCHAIN_PUBLISH_RESULT_CASES.confirmed;
            expect(() =>
                OnchainPublishResultSchema.parse(result),
            ).not.toThrow();
            expect(result.status).toBe("confirmed");
        });

        it("validates a confirmed result with simulation evidence", () => {
            const result: OnchainPublishResult =
                ONCHAIN_PUBLISH_RESULT_CASES.confirmedWithSimulation;
            expect(() =>
                OnchainPublishResultSchema.parse(result),
            ).not.toThrow();
            if (result.status === "confirmed") {
                expect(result.simulationId).toBe("sim-1234567890abcdef");
                expect(result.simulationTraceUrl).toContain("dashboard.tenderly.co");
            }
        });

        it("validates a duplicateRunId result", () => {
            const result: OnchainPublishResult =
                ONCHAIN_PUBLISH_RESULT_CASES.duplicate;
            expect(() =>
                OnchainPublishResultSchema.parse(result),
            ).not.toThrow();
            expect(result.status).toBe("duplicateRunId");
        });

        it("validates a failed result with error", () => {
            const result: OnchainPublishResult =
                ONCHAIN_PUBLISH_RESULT_CASES.failed;
            expect(() =>
                OnchainPublishResultSchema.parse(result),
            ).not.toThrow();
            expect(result.status).toBe("failed");
        });

        it("discriminates result types by status field", () => {
            const confirmed = ONCHAIN_PUBLISH_RESULT_CASES.confirmed;
            const duplicate = ONCHAIN_PUBLISH_RESULT_CASES.duplicate;
            const failed = ONCHAIN_PUBLISH_RESULT_CASES.failed;

            if (confirmed.status === "confirmed") {
                expect(confirmed.txHash).toBeDefined();
                expect(confirmed.blockNumber).toBeDefined();
            }
            if (duplicate.status === "duplicateRunId") {
                expect(duplicate.runId).toBeDefined();
            }
            if (failed.status === "failed") {
                expect(failed.error).toBeDefined();
            }
        });
    });
});
