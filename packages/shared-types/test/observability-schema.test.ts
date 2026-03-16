import { describe, expect, it } from "vitest";

import type {
    CcipDeliveryRecord,
    CcipDeliveryStatus,
    ScoringRunRecord,
} from "../src";

import {
    CcipDeliveryRecordSchema,
    CcipDeliveryStatusSchema,
    ScoringRunRecordSchema,
} from "../src";

import { CCIP_DELIVERY_STATUSES } from "../src";
import {
    ALL_EXPECTED_DELIVERY_STATUSES,
    OBSERVABILITY_CASES,
} from "./fixtures/observability-schema-cases";

describe("observability schema", () => {
    describe("CcipDeliveryStatus", () => {
        it("has exactly the expected delivery statuses", () => {
            expect([...CCIP_DELIVERY_STATUSES]).toEqual(
                ALL_EXPECTED_DELIVERY_STATUSES,
            );
        });

        it("has no duplicate statuses", () => {
            const unique = new Set(CCIP_DELIVERY_STATUSES);
            expect(unique.size).toBe(CCIP_DELIVERY_STATUSES.length);
        });
    });

    describe("ScoringRunRecord", () => {
        it("includes all required ADR fields", () => {
            const run: ScoringRunRecord = OBSERVABILITY_CASES.happyPath.run;

            expect(run.runId).toBeDefined();
            expect(run.customerId).toBeDefined();
            expect("requestId" in run).toBe(true);
            expect("configVersion" in run).toBe(true);
            expect("snapshotUpdatedAt" in run).toBe(true);
            expect(run.status).toBeDefined();
            expect(run.timestamp).toBeDefined();
            expect(run.scenario).toBeDefined();
            expect(run.attemptCount).toBeDefined();
            expect(run.reasonCodes).toBeDefined();
            expect("errorCode" in run).toBe(true);
            expect("errorMessage" in run).toBe(true);
        });

        it("allows null for optional scoring fields before scored state", () => {
            const run: ScoringRunRecord = OBSERVABILITY_CASES.overlapPath.run;

            expect(run.recommendedChain).toBeNull();
            expect(run.score).toBeNull();
            expect(run.reasonCodes).toEqual([]);
        });

        it("populates error fields on failure", () => {
            const run: ScoringRunRecord = OBSERVABILITY_CASES.failurePath.run;

            expect(run.errorCode).toBe("CCIP_SEND_TIMEOUT");
            expect(run.errorMessage).toBe("All 3 CCIP send attempts failed");
            expect(run.attemptCount).toBe(3);
        });
    });

    describe("CcipDeliveryRecord", () => {
        it("includes all required ADR debug fields", () => {
            const delivery: CcipDeliveryRecord =
                OBSERVABILITY_CASES.happyPath.deliveries[0];

            expect(delivery.deliveryId).toBeDefined();
            expect(delivery.runId).toBeDefined();
            expect(delivery.sourceChain).toBeDefined();
            expect(delivery.destinationChain).toBeDefined();
            expect(delivery.status).toBeDefined();
            expect(delivery.attemptCount).toBeDefined();
            expect("ccipMessageId" in delivery).toBe(true);
            expect("errorCode" in delivery).toBe(true);
            expect("errorMessage" in delivery).toBe(true);
        });

        it("allows null ccipMessageId when send has not succeeded", () => {
            const delivery: CcipDeliveryRecord =
                OBSERVABILITY_CASES.failurePath.deliveries[0];

            expect(delivery.ccipMessageId).toBeNull();
        });
    });

    describe("run reconstruction by runId", () => {
        it("joins a ScoringRunRecord with its CcipDeliveryRecords by runId", () => {
            const { run, deliveries } = OBSERVABILITY_CASES.happyPath;

            const matchingDeliveries = deliveries.filter(
                (d: CcipDeliveryRecord) => d.runId === run.runId,
            );

            expect(matchingDeliveries).toHaveLength(1);
            expect(matchingDeliveries[0].runId).toBe(run.runId);
        });

        it("returns no deliveries for a skippedOverlap run", () => {
            const { run, deliveries } = OBSERVABILITY_CASES.overlapPath;

            const matchingDeliveries = deliveries.filter(
                (d: CcipDeliveryRecord) => d.runId === run.runId,
            );

            expect(matchingDeliveries).toHaveLength(0);
        });

        it("delivery status type is assignable to CcipDeliveryStatus", () => {
            const status: CcipDeliveryStatus =
                OBSERVABILITY_CASES.happyPath.deliveries[0].status;

            expect(CCIP_DELIVERY_STATUSES).toContain(status);
        });
    });

    describe("Zod Runtime Validation", () => {
        it("validates a happy path run record", () => {
            const run = OBSERVABILITY_CASES.happyPath.run;
            expect(() => ScoringRunRecordSchema.parse(run)).not.toThrow();
        });

        it("validates a happy path delivery record", () => {
            const delivery = OBSERVABILITY_CASES.happyPath.deliveries[0];
            expect(() => CcipDeliveryRecordSchema.parse(delivery)).not.toThrow();
        });

        it("rejects invalid status in run record", () => {
            const run = { ...OBSERVABILITY_CASES.happyPath.run, status: "invalid-status" };
            expect(() => ScoringRunRecordSchema.parse(run)).toThrow();
        });

        it("rejects invalid status in delivery record", () => {
            const delivery = { ...OBSERVABILITY_CASES.happyPath.deliveries[0], status: "invalid" };
            expect(() => CcipDeliveryRecordSchema.parse(delivery)).toThrow();
        });

        it("rejects missing required fields", () => {
            const run = { ...OBSERVABILITY_CASES.happyPath.run };
            // @ts-expect-error - testing runtime validation
            delete run.runId;
            expect(() => ScoringRunRecordSchema.parse(run)).toThrow();
        });
    });
});
