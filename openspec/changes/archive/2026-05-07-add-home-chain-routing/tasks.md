# Tasks — Home-Chain Routing

## 1. User Service (`services/user-service`)

- [x] Schema migration: add `home_chain BIGINT` column to `users.profiles` (nullable until first deposit).
- [x] gRPC: add `GetUserHomeChain(address) -> (chain_id | not_found)`.
- [x] gRPC: add internal `SetUserHomeChain(address, chain_id)` — used only by first-deposit hook. Idempotent (no-op if already set).
- [x] First-deposit hook lives in **Treasury → user-service gRPC push** (keeps user-service blockchain-unaware per `user-identity/spec.md`):
  - [x] Treasury's existing event indexer detects `Deposited` events.
  - [x] On each event, Treasury calls `SetUserHomeChain(address, chain_id)` on the User Service.
  - [x] `SetUserHomeChain` is idempotent: no-op if `home_chain` is already set.
  - [x] Treasury auth to user-service: existing service-to-service identity (same channel as other internal gRPC calls).
- [x] Tests:
  - [x] First deposit sets home_chain.
  - [x] Second deposit on a different chain does not change home_chain.
  - [x] GetUserHomeChain returns not_found for unknown addresses.

## 2. BFF (`services/bff`)

- [x] When building a `transferHotPath` payload, resolve recipient's home chain via `GetUserHomeChain`.
- [x] Apply fallback policy:
  - [x] not_found → use sender's chain.
  - [x] home_chain inactive in `RouteReceiver` → use sender's chain.
  - [x] User Service unavailable → use sender's chain (and log).
- [x] Pass resolved `destChainId` in the GraphQL response so the frontend signs with the correct value.
- [x] Tests covering each fallback path.

## 3. Frontend (`apps/bank-client`)

- [x] Use the `destChainId` returned by the BFF when constructing the EIP-2718 payload.
- [x] No UI change — user does not see the chain.

## 4. Treasury (`services/treasury`)

- [x] No code changes. The hot path already accepts arbitrary `destChainId`.

## 5. On-chain (`packages/onchain`)

- [x] No changes.

## 6. Documentation

- [x] Update `docs/user-journey.md` step 3 / step 4 to reflect home-chain delivery for incoming cross-chain transfers.
- [x] Note in operational docs: `home_chain` is set automatically and is not user-visible.
