# Tasks — Account Balance Aggregation and Activity Feed

## 1. Treasury — schema and event index

- [ ] 1.1 Migration: create `treasury.account_events` (`chain_id`, `tx_hash`, `log_index`, `event_kind`, `address_from`, `address_to`, `amount_wei`, `block_number`, `block_time`, `correlation`; unique on `(chain_id, tx_hash, log_index)`) and `treasury.index_cursors` (`chain_id`, `last_block`).
- [ ] 1.2 New `account_index.rs` module: per-chain `eth_getLogs` polling for `Deposited`, `Withdrawn`, SyncUSD `Transfer`, `HotPathInitiated`, `HotPathReleased`, with idempotent upserts and persisted cursors.
- [ ] 1.3 Repository trait + `PgAccountEventRepository` (domain/repository.rs, db/) following the existing repo pattern.
- [ ] 1.4 Refactor `home_chain.rs`: drop its own polling and in-memory cursor; trigger `SetUserHomeChain` from first indexed `Deposited` per address (restart-safe, no duplicate pushes).
- [ ] 1.5 Tests: idempotent ingestion (duplicate log replay), cursor resume after restart, home-chain push fires exactly once.

## 2. Treasury — balance aggregation

- [ ] 2.1 `balanceOf(address)` `eth_call` helper in `eth/` (encode selector + address, decode uint256).
- [ ] 2.2 Pure aggregation function: fold per-chain results, skipping decommissioned chains; unit-test the fold including the degraded-substitution case.
- [ ] 2.3 Re-implement `GetBalance`: concurrent fan-out over non-decommissioned chains, fall back to last indexed balance per failed chain, set a `degraded` flag in the response; delete `relay_repo.get_balance`.
- [ ] 2.4 In-process per-address cache (3–5s TTL) for balance reads.

## 3. Treasury — activity RPC

- [ ] 3.1 Proto: add `GetAccountActivity(address, limit)` returning `ActivityEntry { kind, direction, counterparty, chain_id, amount_wei, status, tx_hash, occurred_at }`; mark `GetRecentTransfers` deprecated.
- [ ] 3.2 Implement `get_account_activity`: query `account_events` where the user is sender or recipient; collapse hot-path pairs (sender sees initiated + relay status, recipient sees released); exclude internal Bank/pool movements.
- [ ] 3.3 Tests: feed contains deposit, withdrawal, same-chain transfer both directions, hot-path both directions; rebalance/reserve movements excluded.

## 4. BFF

- [ ] 4.1 Treasury port + gRPC adapter: add `getAccountActivity`; switch `recentTransfers` resolver to it, mapping into the existing GraphQL `Transfer` type plus new optional `kind`/`direction` fields.
- [ ] 4.2 `balance` resolver: pass through new semantics; surface `degraded` as an optional field if cheap.
- [ ] 4.3 Remove BFF usage of `GetRecentTransfers`; then delete the RPC from proto and treasury server.

## 5. Verification

- [ ] 5.1 E2E (local anvil/docker-compose): deposit → balance shows amount; send same-chain → both feeds update; hot-path send → sender debited, recipient credited, both feeds show the transfer.
- [ ] 5.2 Kill one chain RPC and confirm balance degrades gracefully instead of erroring.
- [ ] 5.3 Update `docs/user-journey.md` note: dashboard balance is served by BFF `balance` (Treasury aggregation), not by client-side wagmi reads.
