# Tasks — USDC Reserve Rebalance

## 1. On-chain interface (`packages/onchain`)

- [x] Define `IReserveBridge` interface:
  - [x] `bridgeOut(uint64 destChainId, uint256 amount, address destReserve) returns (bytes32 messageId)`
  - [x] `bridgeIn(...)` callback (signature per bridge type — abstract in interface).
- [x] Implement `CCTPReserveBridge` adapter for CCTP-supported chains:
  - [x] Wraps Circle's `TokenMessenger.depositForBurn` and `MessageTransmitter.receiveMessage`.
  - [x] Unit tests with mocked CCTP contracts (23 tests in `CCTPReserveBridge.t.sol`).
  - [ ] Integration tests against forked Base/Arbitrum (deferred — tracked under section 3 alongside Treasury fork test).
- [ ] Implement Tempo custom reserve bridge:
  - [ ] Decide LayerZero vs. Wormhole as messaging protocol (sub-decision).
  - [ ] Adapter sends release intent on source via the chosen protocol.
  - [ ] Multisig signs and submits release on destination Tempo Bank Contract.
  - [ ] Replay protection via per-message nonce.
  - [ ] Tests against forked Tempo + a CCTP chain.

## 2. On-chain Bank Contract (`packages/onchain`)

- [x] Add `RESERVE_REBALANCER_ROLE` constant.
- [x] Add storage:
  - [x] `reserveBridge` (IReserveBridge)
  - [x] `maxReserveRebalanceAmount` (uint256)
  - [x] `processedReserveMessages` (mapping bytes32 => bool) for idempotency on inbound.
- [x] Admin functions (governance/admin role):
  - [x] `setReserveBridge(IReserveBridge)`
  - [x] `setMaxReserveRebalanceAmount(uint256)`
- [x] Add `bridgeReserve(uint64 destChainId, uint256 amount)`:
  - [x] Requires `RESERVE_REBALANCER_ROLE`.
  - [x] Reverts if `amount > maxReserveRebalanceAmount`.
  - [x] Reverts if `destChainId` is inactive in `RouteReceiver`.
  - [x] Reverts if reserve depth < amount.
  - [x] Approves `reserveBridge` to pull USDC.
  - [x] Calls `reserveBridge.bridgeOut(...)`, captures `messageId`.
  - [x] Emits `ReserveBridgeInitiated(destChainId, amount, messageId, bridgeType)`.
- [x] Add inbound handler called by `reserveBridge`:
  - [x] Verifies caller is the registered `reserveBridge`.
  - [x] Reverts on already-processed `messageId`.
  - [x] Marks `messageId` processed.
  - [x] Adds USDC to reserve (no-op since the bridge already minted/released into the contract; just records the credit).
  - [x] Emits `ReserveBridgeCompleted(srcChainId, amount, messageId)`.
- [x] Add `reserveDepth()` view returning current USDC reserve balance.
- [x] Foundry tests:
  - [x] Happy path round-trip with mock adapter.
  - [x] Unauthorized caller: reverts.
  - [x] Amount > cap: reverts.
  - [x] Inactive destination: reverts.
  - [x] Replay: reverts on second delivery.
  - [x] Adapter not set: reverts cleanly.

## 3. Treasury Service (`services/treasury`)

- [x] Migration: create `treasury.reserve_ops` table (`migrations/20260511000000_reserve_ops.sql`).
- [x] New module `reserve_path.rs`:
  - [x] Polls `reserveDepth()` on each Bank Contract (planner loop).
  - [x] Computes per-chain target deviation against `RESERVE_PATH_MIN_BPS` of `total / n` target.
  - [x] Plans bridge operations (reuses `compute_rebalance_ops`; respects `RouteReceiver` activation gate).
  - [x] Calls `bridgeReserve` and persists `messageId` to `reserve_ops`.
  - [x] Relayer loop: fetches Circle attestation via `iris-api`, dispatches `bridgeIn` on destination adapter.
  - [x] Watcher loop: scans destination chains for `ReserveBridgeCompleted` events, updates row to `completed`.
  - [x] Times out stuck ops past `RESERVE_PATH_STUCK_TIMEOUT_SECS` and marks `failed`.
- [ ] Integration test against forked CCTP Base + Arbitrum (deferred — requires testnet RPC + funded keys; tracked under section 5 deployment).

## 4. CRE / RouteReceiver

- [x] No changes. Reserve rebalance reads existing activation state.

## 5. Configuration & deployment

- [ ] Deploy `CCTPReserveBridge` on each CCTP-supported chain.
- [ ] Deploy Tempo custom bridge adapter + destination-side multisig.
- [ ] Governance multisig: register adapter address per Bank Contract via `setReserveBridge`.
- [ ] Set `maxReserveRebalanceAmount` per chain: **5% of total USDC reserves**.
- [ ] Set Treasury reserve-threshold config: target = `total_reserve / num_active_chains`, drain trigger at **80% of target**, surplus source at **>100% of target**.
- [ ] Set Treasury stuck-message timeout: **30 min for CCTP, 60 min for Tempo custom bridge** (manual operator review on timeout).
- [ ] Grant `RESERVE_REBALANCER_ROLE` to Treasury reserve-ops signer (separate key from cold-path signer).

## 6. Documentation

- [ ] Add operational runbook entry: how to investigate a stuck reserve bridge.
- [ ] Update `docs/user-journey.md` step 6 once reactive failover is in place (note: this change alone does not deliver reactive failover).
