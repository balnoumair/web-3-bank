# Design — Withdrawal Failover

## Context

The journey doc promises withdrawals are "fulfilled from healthy pools" when a user's chain is down, but the system is non-custodial: only the user's passkey can burn the user's SyncUSD, and the burn must execute on the chain holding the tokens. If that chain is not processing transactions, neither Treasury nor anyone else can move those funds without custody or a pre-signed authorization scheme. The promise as written is unimplementable in the current trust model — the design question is what to promise instead.

## Goals / Non-Goals

**Goals:**
- A user can always withdraw whatever portion of their balance sits on chains that can process transactions.
- The client never builds a `withdraw()` transaction destined to fail or hang; it learns the viable chain(s) first.
- The docs, specs, and code make the same promise.

**Non-Goals:**
- Custodial fulfillment from other pools (violates the non-custodial model).
- Pre-signed withdrawal authorizations / meta-transactions for dead chains (interesting, large, separate proposal if ever).
- Changing `withdraw()` on the Bank Contract.

## Decisions

### 1. Failover = route the withdrawal, don't fake fulfillment
The withdrawable amount is per-chain: `min(balanceOf(user, chain), reserveDepth(chain))` on chains that are active (or degraded-but-processing). The BFF exposes this breakdown; the client withdraws where it can. Balance stuck on an inactive chain is reported as **temporarily unavailable** with an explicit reason — recovered by chain recovery or by decommission drain (which moves holder balances to a healthy chain through the already-specced procedure).

*Alternative considered — Treasury-fronted fulfillment:* Treasury pays the user from a healthy pool now and reimburses itself from the user's stranded SyncUSD later. Rejected: it is a credit decision (Treasury takes default risk on a chain that may never recover), requires custody-like claims on user balances, and silently converts the bank from non-custodial to fractional. If product ever wants this, it needs its own proposal with explicit risk limits.

### 2. Per-chain withdrawability comes from existing reads
All inputs exist after `add-account-balance-and-activity`: per-chain `balanceOf` (balance fan-out), reserve depth (`reserveDepth` on-chain read, already specced), activation state (RouteReceiver via `IsChainActive`). The new RPC is composition, not new infrastructure.

### 3. "Degraded but processing" is the activation flag, nothing fancier
A chain is withdrawable iff it is active in RouteReceiver and not decommissioned. We do not invent a third "limping" classification — CRE's scoring already encodes processability into activation.

### 4. Journey doc gets corrected, not the model bent to the doc
§6's withdrawal sentence is rewritten: deposits and sends route around dead chains (true today); withdrawals are available for funds on healthy chains, and funds on a dead chain are safe but frozen until recovery or decommission. Honest > magical.

## Risks / Trade-offs

- [Users with their entire balance on a dead chain see $X "unavailable"] → this is the truthful state of a non-custodial system; surfaced with a reason string and covered by decommission for permanent failures. Mitigation for the future: home-chain stickiness already concentrates user funds on chains CRE considers healthy.
- [Reserve depth on the healthy chain may cap a large withdrawal] → already specced (reserve rebalance keeps reserves funded); the routing response exposes the cap honestly instead of letting the tx revert.
- [Frontend complexity: multi-chain withdraw] → the client already handles per-chain sends (`transferHotPath` vs `transfer`); withdraw chain selection follows the same pattern, invisible to the user.

## Migration Plan

1. Land after `add-account-balance-and-activity`.
2. Add `GetWithdrawalRouting` to Treasury proto + BFF query; client consults it in `use-withdraw`.
3. Rewrite journey §6; archive updates specs.

## Open Questions

- Display: should "unavailable" balance be folded into the single USD figure with a badge, or shown separately? (Product/frontend call — flagged for the architect.)
- Should withdrawals on a degraded-but-active chain warn the user about potential slowness? (Default: no — invisible chains is the product thesis.)
