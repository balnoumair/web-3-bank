# Design — Home-Chain Routing

## Goals

- Incoming cross-chain transfers consolidate on the recipient's home chain by default.
- The mechanism uses the existing hot path with no contract changes.
- Routing decisions degrade gracefully when the home chain is inactive — never block the transfer entirely.

## Non-goals

- Eliminating split balances entirely. Past balances are preserved as-is. Only future receives route home.
- User-facing chain controls.
- Autonomous movement of user-owned SyncUSD between chains.

## Key design decisions

### 1. Home chain is set on first deposit, then sticky

Rationale: the user's first deposit is the strongest signal of their "primary" chain — it's the chain they (or their wallet) chose to onboard on. Reassigning later in response to transient chain health would cause balance scatter as the home chain flaps. Sticky avoids this.

The only mechanism that may change `home_chain` after creation is chain decommissioning (separate change), which is a one-shot governance event, not a continuous policy.

### 2. Fallback to sender's chain when home is unreachable

Three fallback cases, all resolve to sender's chain (same-chain delivery):

| Case | Fallback |
|---|---|
| Recipient has no User Service record (raw address) | Same-chain |
| Recipient's home chain is inactive in `RouteReceiver` | Same-chain |
| User Service is unreachable (degraded) | Same-chain |

Same-chain is always available (the sender just executed on it). It's never the worst answer. The cost is a temporarily-split balance that will likely live on the sender's chain until the recipient sends or withdraws.

### 3. Resolution lives in the BFF, not on-chain or in the contract

The Bank Contract has no concept of users or home chains. It just takes a `destChainId`. Putting the resolution in the BFF keeps the contract minimal and lets routing policy evolve (caching, A/B, alternative heuristics) without contract upgrades.

### 4. No fallback within the User Service for other users' home chain

`GetUserHomeChain` returns the stored value or "not found". The User Service does not implement fallback policy — that belongs to the BFF, which has full context (sender's chain, RouteReceiver state, etc.).

## Decisions

- **First-deposit hook owner: Treasury, pushing to user-service via gRPC.** User-service must stay blockchain-unaware (per `user-identity/spec.md`), so it cannot index events itself. Treasury already runs an event indexer for the watcher; adding a `SetUserHomeChain` push is incremental.
- **Re-onboarding (account delete + recreate): out of scope.** Re-creation flows are not specified anywhere in the system today. Treat the first-ever deposit as setting `home_chain`; do not redesign for a flow that doesn't exist.

## Open questions

- **First-deposit semantics for cross-chain deposits.** Today, `deposit()` is a same-chain operation (USDC in, SyncUSD out, on the same chain). Home chain is unambiguous: it's the deposit chain. If multi-chain deposit flows are added later, this becomes ambiguous and will need revisiting.
