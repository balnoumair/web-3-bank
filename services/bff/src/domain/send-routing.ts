/**
 * Pure routing policy for hot-path `destinationChainId`.
 * IO (user-service, treasury) is handled by callers; this encodes fallbacks only.
 */

export function resolveDestChainId(input: {
  senderChainId: bigint;
  /** Undefined when user has no stored home chain (or not onboarded). */
  recipientHomeChainId?: bigint;
  /** Whether RouteReceiver marks the home chain active (same view as relayer). */
  recipientHomeChainActive: boolean;
  /** Whether governance has permanently retired the home chain. */
  recipientHomeChainDecommissioned?: boolean;
}): bigint {
  const {
    senderChainId,
    recipientHomeChainId,
    recipientHomeChainActive,
    recipientHomeChainDecommissioned = false,
  } = input;
  if (recipientHomeChainId === undefined) {
    return senderChainId;
  }
  if (recipientHomeChainDecommissioned) {
    return senderChainId;
  }
  if (!recipientHomeChainActive) {
    return senderChainId;
  }
  return recipientHomeChainId;
}
