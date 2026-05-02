# Add Home-Chain Routing for Incoming Transfers

## Why

Today, when Alice on Base sends Bob $500 via the hot path, Bob receives 500 SyncUSD on Base — regardless of where Bob's existing balance lives. Over time, frequent recipients accumulate small balances scattered across every chain a sender happened to be on. This breaks two product expectations:

- **"Withdraw all"** becomes awkward: a user with $5,000 split across three chains needs three transactions.
- **The single-balance UX is a half-truth.** During chain outages, the user's "$5,500 balance" is partly stranded.

Routing every incoming cross-chain transfer to the recipient's **home chain** keeps each user's balance consolidated by default.

## What changes

1. **Add `home_chain` to the `users` schema.** Set on first deposit (whichever chain the deposit landed on becomes home). Stored and managed by the User Service.
2. **Add gRPC method** `GetUserHomeChain(address)` on the User Service so senders can resolve a recipient's home chain.
3. **BFF resolves the recipient's home chain** when building a hot-path transfer and passes it as `destChainId` in `transferHotPath`.
4. **Fallback rules:**
   - Recipient is a raw address with no User Service record → use sender's chain (same-chain delivery).
   - Recipient's home chain is inactive in `RouteReceiver.sol` → use sender's chain (same-chain delivery). Stranded balance accumulates on sender's chain temporarily; recovers when home chain returns.
5. **`home_chain` is system-managed only.** No user-facing setting. Users do not see chains. The User Service SHALL NOT mutate `home_chain` in response to chain-health changes (sticky preference per design discussion).

## Out of scope

- **No on-chain changes.** `transferHotPath` already accepts `destChainId`; this change is purely about who decides the value.
- **Consolidation of pre-existing split balances.** Past balances stay where they are. Future incoming transfers will route home.
- **Auto-reassignment of `home_chain` on chain inactivity.** Sticky preference, not dynamic. Reassignment only happens via the chain-decommissioning procedure (separate change).
- **User-initiated home-chain change.** Out of scope; the product premise hides chains from users.

## Dependencies

- None on `add-cold-path-rebalance` — this change is independent and can ship first.
- `add-chain-decommissioning` will introduce the only path that mutates `home_chain` after creation.

## Impact

- `services/user-service` — schema migration (add `home_chain` column), gRPC method, set on first-deposit hook.
- `services/bff` — resolve recipient `home_chain` before building hot-path transfer.
- `apps/bank-client` — minor: pass through the resolved `destChainId`.
- `services/treasury` — no change.
- `packages/onchain` — no change.
