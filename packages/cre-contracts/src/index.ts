import type { RouteUpdated } from "@repo/shared-types";

export type RouteUpdatedEnvelope = {
  sourceChain: "ethereum-sepolia";
  destinationChain: "base-sepolia";
  payload: RouteUpdated;
};

export function createRouteUpdatedEnvelope(
  payload: RouteUpdated,
): RouteUpdatedEnvelope {
  return {
    sourceChain: "ethereum-sepolia",
    destinationChain: "base-sepolia",
    payload,
  };
}
