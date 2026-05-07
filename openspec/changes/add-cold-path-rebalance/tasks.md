# Tasks — Cold Path Pool Rebalance

## 1. On-chain (`packages/onchain`)

- [x] Add `REBALANCER_ROLE` constant and AccessControl wiring to `Bank.sol`.
- [x] Add storage:
  - [x] `maxRebalanceAmount` (uint256)
  - [x] `allowlistedDestChains` (mapping uint64 => bool)
  - [x] `allowlistedSourceContracts` (mapping (uint64, address) => bool)
  - [x] `processedMessages` (mapping bytes32 => bool) for inbound CCIP idempotency
- [x] Add admin functions (governance/admin role):
  - [x] `setMaxRebalanceAmount(uint256)`
  - [x] `setAllowlistedDestChain(uint64, bool)`
  - [x] `setAllowlistedSourceContract(uint64, address, bool)`
- [ ] Add `rebalance(uint64 destChainId, uint256 amount)`:
  - [x] Requires `REBALANCER_ROLE`.
  - [x] Reverts if `amount > maxRebalanceAmount`.
  - [x] Reverts if `destChainId` not allowlisted.
  - [x] Reverts if pool depth < `amount`.
  - [x] Burns `amount` SyncUSD from the local pool (via CCIP token pool burn path).
  - [ ] Sends CCIP message to destination Bank Contract carrying `(amount, messageId)`.
  - [x] Emits `RebalanceInitiated(destChainId, amount, messageId)`.
- [x] Add `_ccipReceive` handler:
  - [x] Verifies sender chain + sender contract are allowlisted.
  - [x] Reverts if `messageId` already in `processedMessages`.
  - [x] Marks `messageId` as processed.
  - [x] Mints `amount` SyncUSD to local pool.
  - [x] Emits `RebalanceCompleted(srcChainId, amount, messageId)`.
- [x] Foundry tests:
  - [x] Happy path: source burns, destination mints, events match, pool depths update.
  - [x] Unauthorized caller (no `REBALANCER_ROLE`): reverts.
  - [x] Amount > cap: reverts.
  - [x] Destination not allowlisted: reverts.
  - [x] Pool depth insufficient: reverts.
  - [x] Replay (same `messageId` delivered twice): second delivery reverts.
  - [x] Inbound from non-allowlisted source: reverts.
  - [x] Pause: while contract is paused, `rebalance` reverts (existing `Pausable`).

## 2. Treasury Service (`services/treasury`)

- [x] Regenerate Bank.sol ABI bindings (alloy `sol!` macro or generated).
- [x] Update `cold_path.rs::execute_rebalance` to call the new function selector.
- [x] Read `maxRebalanceAmount` from the source contract; if Treasury's planned op exceeds it, split into sequential ops.
- [x] Capture the CCIP `messageId` from the receipt (via emitted event) and persist to `treasury.rebalance_ops`.
- [x] Add error mapping for new revert reasons:
  - [x] `RebalanceCapExceeded` → log and split.
  - [x] `DestChainNotAllowlisted` → operator alert.
  - [x] `PoolDepthInsufficient` → reschedule.
- [ ] Integration test against forked Tempo + Base:
  - [ ] Triggers rebalance, verifies destination pool minted, audit row written.
  - [ ] Replay of same op: rejected on destination, audit row records failure.

## 3. CRE / RouteReceiver

- [x] No changes. Cold path consults `RouteReceiver` via existing read path.

## 4. Configuration & deployment

- [x] Set initial `maxRebalanceAmount` per chain (governance multisig tx): **5% of total SyncUSD supply**.
- [x] Set Treasury pool-threshold config: target = `total_supply / num_active_chains`, drain trigger at **80% of target**, surplus source at **>100% of target**.
- [x] Set Treasury stuck-message timeout: **30 minutes** (manual operator review on timeout).
- [x] Allowlist each pair of Bank Contracts as valid CCIP source/destination.
- [x] Grant `REBALANCER_ROLE` to Treasury signer addresses (per chain).

## 5. Documentation

- [x] Update `docs/user-journey.md` step 5 if any details now differ.
- [x] Add operational runbook entry:
  - [x] How to pause cold path (existing `Pausable` mechanism).
  - [x] How to adjust `maxRebalanceAmount`.
  - [x] How to investigate a stuck CCIP message.

## 6. Out-of-scope tracking (for future changes)

- [x] Open follow-up proposal: `add-usdc-reserve-rebalance` (Circle CCTP or equivalent) — required for true withdraw failover.
- [x] Open follow-up proposal: `add-chain-decommissioning` — governance procedure for permanent chain removal.
- [x] Open follow-up proposal: `add-home-chain-routing` — receive-on-home-chain routing.
