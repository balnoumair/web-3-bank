# Cross-Chain Routing — Delta

## MODIFIED Requirements

### Requirement: Cold Path rebalances via CCIP burn-and-mint

When a chain's pool drops below its target threshold, the Treasury Service SHALL invoke `rebalance(destChainId, amount)` on a surplus chain's Bank Contract. The Bank Contract SHALL burn the amount locally and SHALL trigger a CCIP burn-and-mint operation that mints the same amount on the destination chain's Bank Contract. Cold path operations SHALL be invisible to end users.

The Treasury Service SHALL NOT submit a rebalance whose destination chain is marked inactive in `RouteReceiver.sol`.

The cold path SHALL only rebalance the **SyncUSD pool** held by Bank Contracts. It SHALL NOT move underlying USDC reserves.

## ADDED Requirements

### Requirement: Audit trail for rebalances

Every cold path rebalance attempt SHALL be recorded in the `treasury.rebalance_ops` table, keyed on the CCIP `messageId`. The record SHALL include source chain, destination chain, amount, status (`initiated`, `completed`, `failed`), and any revert reason. Recording SHALL occur regardless of outcome — including when the on-chain call reverts before a `messageId` is produced (in which case `messageId` is null and the row is keyed on a Treasury-side correlation id).

### Requirement: Cold path respects per-rebalance cap

The Treasury Service SHALL read the destination Bank Contract's `maxRebalanceAmount` and SHALL split any logical rebalance whose total amount exceeds the cap into multiple sequential operations, each within the cap. The on-chain cap is authoritative — Treasury MUST NOT submit operations exceeding it. If Treasury's local cap configuration drifts above the on-chain cap, the on-chain check SHALL revert and Treasury SHALL log and re-plan.
