# Tasks — Home-Chain Routing

## 1. User Service (`services/user-service`)

- [ ] Schema migration: add `home_chain BIGINT` column to `users.profiles` (nullable until first deposit).
- [ ] gRPC: add `GetUserHomeChain(address) -> (chain_id | not_found)`.
- [ ] gRPC: add internal `SetUserHomeChain(address, chain_id)` — used only by first-deposit hook. Idempotent (no-op if already set).
- [ ] First-deposit hook lives in **Treasury → user-service gRPC push** (keeps user-service blockchain-unaware per `user-identity/spec.md`):
  - [ ] Treasury's existing event indexer detects `Deposited` events.
  - [ ] On each event, Treasury calls `SetUserHomeChain(address, chain_id)` on the User Service.
  - [ ] `SetUserHomeChain` is idempotent: no-op if `home_chain` is already set.
  - [ ] Treasury auth to user-service: existing service-to-service identity (same channel as other internal gRPC calls).
- [ ] Tests:
  - [ ] First deposit sets home_chain.
  - [ ] Second deposit on a different chain does not change home_chain.
  - [ ] GetUserHomeChain returns not_found for unknown addresses.

## 2. BFF (`services/bff`)

- [ ] When building a `transferHotPath` payload, resolve recipient's home chain via `GetUserHomeChain`.
- [ ] Apply fallback policy:
  - [ ] not_found → use sender's chain.
  - [ ] home_chain inactive in `RouteReceiver` → use sender's chain.
  - [ ] User Service unavailable → use sender's chain (and log).
- [ ] Pass resolved `destChainId` in the GraphQL response so the frontend signs with the correct value.
- [ ] Tests covering each fallback path.

## 3. Frontend (`apps/bank-client`)

- [ ] Use the `destChainId` returned by the BFF when constructing the EIP-2718 payload.
- [ ] No UI change — user does not see the chain.

## 4. Treasury (`services/treasury`)

- [ ] No code changes. The hot path already accepts arbitrary `destChainId`.

## 5. On-chain (`packages/onchain`)

- [ ] No changes.

## 6. Documentation

- [ ] Update `docs/user-journey.md` step 3 / step 4 to reflect home-chain delivery for incoming cross-chain transfers.
- [ ] Note in operational docs: `home_chain` is set automatically and is not user-visible.
