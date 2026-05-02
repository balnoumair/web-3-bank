# Tasks — Cold Path Pool Rebalance

## 1. On-chain (`packages/onchain`)

- [ ] Add `REBALANCER_ROLE` constant and AccessControl wiring to `Bank.sol`.
- [ ] Add storage:
  - [ ] `maxRebalanceAmount` (uint256)
  - [ ] `allowlistedDestChains` (mapping uint64 => bool)
  - [ ] `allowlistedSourceContracts` (mapping (uint64, address) => bool)
  - [ ] `processedMessages` (mapping bytes32 => bool) for inbound CCIP idempotency
- [ ] Add admin functions (governance/admin role):
  - [ ] `setMaxRebalanceAmount(uint256)`
  - [ ] `setAllowlistedDestChain(uint64, bool)`
  - [ ] `setAllowlistedSourceContract(uint64, address, bool)`
- [ ] Add `rebalance(uint64 destChainId, uint256 amount)`:
  - [ ] Requires `REBALANCER_ROLE`.
  - [ ] Reverts if `amount > maxRebalanceAmount`.
  - [ ] Reverts if `destChainId` not allowlisted.
  - [ ] Reverts if pool depth < `amount`.
  - [ ] Burns `amount` SyncUSD from the local pool (via CCIP token pool burn path).
  - [ ] Sends CCIP message to destination Bank Contract carrying `(amount, messageId)`.
  - [ ] Emits `RebalanceInitiated(destChainId, amount, messageId)`.
- [ ] Add `_ccipReceive` handler:
  - [ ] Verifies sender chain + sender contract are allowlisted.
  - [ ] Reverts if `messageId` already in `processedMessages`.
  - [ ] Marks `messageId` as processed.
  - [ ] Mints `amount` SyncUSD to local pool.
  - [ ] Emits `RebalanceCompleted(srcChainId, amount, messageId)`.
- [ ] Foundry tests:
  - [ ] Happy path: source burns, destination mints, events match, pool depths update.
  - [ ] Unauthorized caller (no `REBALANCER_ROLE`): reverts.
  - [ ] Amount > cap: reverts.
  - [ ] Destination not allowlisted: reverts.
  - [ ] Pool depth insufficient: reverts.
  - [ ] Replay (same `messageId` delivered twice): second delivery reverts.
  - [ ] Inbound from non-allowlisted source: reverts.
  - [ ] Pause: while contract is paused, `rebalance` reverts (existing `Pausable`).

## 2. Treasury Service (`services/treasury`)

- [ ] Regenerate Bank.sol ABI bindings (alloy `sol!` macro or generated).
- [ ] Update `cold_path.rs::execute_rebalance` to call the new function selector.
- [ ] Read `maxRebalanceAmount` from the source contract; if Treasury's planned op exceeds it, split into sequential ops.
- [ ] Capture the CCIP `messageId` from the receipt (via emitted event) and persist to `treasury.rebalance_ops`.
- [ ] Add error mapping for new revert reasons:
  - [ ] `RebalanceCapExceeded` → log and split.
  - [ ] `DestChainNotAllowlisted` → operator alert.
  - [ ] `PoolDepthInsufficient` → reschedule.
- [ ] Integration test against forked Tempo + Base:
  - [ ] Triggers rebalance, verifies destination pool minted, audit row written.
  - [ ] Replay of same op: rejected on destination, audit row records failure.

## 3. CRE / RouteReceiver

- [ ] No changes. Cold path consults `RouteReceiver` via existing read path.

## 4. Configuration & deployment

- [ ] Set initial `maxRebalanceAmount` per chain (governance multisig tx): **5% of total SyncUSD supply**.
- [ ] Set Treasury pool-threshold config: target = `total_supply / num_active_chains`, drain trigger at **80% of target**, surplus source at **>100% of target**.
- [ ] Set Treasury stuck-message timeout: **30 minutes** (manual operator review on timeout).
- [ ] Allowlist each pair of Bank Contracts as valid CCIP source/destination.
- [ ] Grant `REBALANCER_ROLE` to Treasury signer addresses (per chain).

## 5. Documentation

- [ ] Update `docs/user-journey.md` step 5 if any details now differ.
- [ ] Add operational runbook entry:
  - [ ] How to pause cold path (existing `Pausable` mechanism).
  - [ ] How to adjust `maxRebalanceAmount`.
  - [ ] How to investigate a stuck CCIP message.

## 6. Out-of-scope tracking (for future changes)

- [ ] Open follow-up proposal: `add-usdc-reserve-rebalance` (Circle CCTP or equivalent) — required for true withdraw failover.
- [ ] Open follow-up proposal: `add-chain-decommissioning` — governance procedure for permanent chain removal.
- [ ] Open follow-up proposal: `add-home-chain-routing` — receive-on-home-chain routing.
