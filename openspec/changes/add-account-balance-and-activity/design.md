# Design — Account Balance Aggregation and Activity Feed

## Context

The dashboard balance and activity feed are served by `TreasuryService.GetBalance` / `GetRecentTransfers`, both reading `treasury.relay_logs` — a hot-path **audit** table, never designed as an account ledger. It records only relays (and only filters on `recipient`). Meanwhile the truth lives on-chain: SyncUSD is an ERC-20/TIP-20 token, so `balanceOf(address)` per chain is authoritative, and every flow the user cares about already emits an event (`Deposited`, `Withdrawn`, `Transfer`, `HotPathInitiated`, `HotPathReleased`).

Treasury already has the building blocks: per-chain RPC plumbing (`eth/rpc.rs`), `eth_getLogs` polling with block cursors (`home_chain.rs`), and a Postgres schema it owns. The home-chain indexer polls `Deposited` events today but throws them away after calling `SetUserHomeChain`.

Constraints:
- Service boundaries: only Treasury talks to chains; BFF stays a thin proxy; User Service stays off-chain.
- DDD: indexing (infrastructure) must stay separate from the read-model logic (domain).
- Functional style: balance aggregation is a pure fold over per-chain reads.

## Goals / Non-Goals

**Goals:**
- A user's displayed balance equals the sum of their on-chain SyncUSD across all non-decommissioned chains.
- The activity feed shows every flow that affects the user: deposits, withdrawals, same-chain transfers (sent/received), hot-path transfers (sent/received), with status and tx hash.
- A persistent event index that the decommission drain's `HolderIndexPort` can later consume (holder enumeration).
- Graceful degradation: if one chain's RPC is down, balance is served from the freshest indexed data rather than failing.

**Non-Goals:**
- Push notifications / websockets (frontend keeps polling).
- Fiat formatting, statements, exports — presentation concerns.
- Re-pricing or interest; SyncUSD is 1:1 USD by definition.
- Replacing `relay_logs` — it remains the hot-path audit trail.

## Decisions

### 1. Balance = live `eth_call balanceOf` fan-out, indexed events as fallback
For each non-decommissioned chain, Treasury issues `balanceOf(address)` via `eth_call` concurrently and sums the results (pure fold). If a chain RPC fails, substitute that chain's last indexed balance and mark the response `degraded: true`.

*Alternative considered:* compute balance purely from the event index (no RPC at read time). Rejected as primary source — index lag would show stale balances right after a deposit, the exact moment users watch the number. The 10s polling + ~0.5s chains make live reads cheap; per-address result can be cached for a few seconds in-process.

### 2. One `treasury.account_events` table, one indexer, home-chain logic becomes a consumer
New table keyed `(chain_id, tx_hash, log_index)` (idempotent upserts), columns: `event_kind`, `address_from`, `address_to`, `amount_wei`, `block_number`, `block_time`, `correlation` (e.g. `sourceEventHash` for hot-path pairs). One polling indexer per chain with a persisted block cursor (`treasury.index_cursors`), replacing the in-memory `HashMap<ChainId, u64>` in `home_chain.rs` (which currently re-scans from scratch on restart). The first indexed `Deposited` for an address triggers the existing `SetUserHomeChain` call — same behavior, now restart-safe.

*Alternative considered:* keep two separate indexers (home-chain + activity). Rejected: same events, same cursor problem, double RPC load.

### 3. Activity feed served from the index, hot-path pairs collapsed
`GetAccountActivity(address, limit)` returns entries where the user is sender **or** recipient. A hot-path transfer appears once for the sender (`HotPathInitiated`, status from the matching relay) and once for the recipient (`HotPathReleased`). Same-chain `Transfer` events where `from`/`to` is the Bank Contract or the CCIP pool are tagged internal and excluded from the user feed.

*Alternative considered:* extend `GetRecentTransfers` in place. Rejected: its `TransferRecord` shape (relay-centric: `source_event_hash`, relay `status`) doesn't fit deposits/withdrawals; a typed `ActivityEntry { kind, direction, counterparty, chain_id, amount_wei, status, tx_hash, occurred_at }` is clearer. `GetRecentTransfers` stays temporarily and is removed once the BFF migrates.

### 4. BFF GraphQL shape stays backward-compatible
`balance` keeps returning a string (now correct). `recentTransfers` maps `ActivityEntry` into the existing `Transfer` GraphQL type plus new optional fields (`kind`, `direction`). The frontend needs no changes to become correct; it can adopt the new fields later.

## Risks / Trade-offs

- [RPC fan-out cost: every balance poll hits N chains] → in-process per-address cache (3–5s TTL) keeps worst case at ~N calls per user per TTL; later, swap polling for the indexer's balance materialization if cost grows.
- [Index lag makes activity briefly incomplete after a tx] → acceptable: balance is live; feed catching up seconds later matches user expectations (banks show pending items late, not balances wrong).
- [`Transfer` event volume on busy chains] → index only events touching known Bank/SyncUSD contracts (already the case via `eth_getLogs` address filter); prune/partition by `block_time` if the table grows.
- [Double counting in-flight hot-path funds: sender already debited on source, recipient not yet credited on destination] → correct by construction: `balanceOf` sums show the debit immediately and the credit on release; the feed shows the pending entry via relay status.
- [Behavioral break for anything relying on old `GetBalance` semantics] → only the BFF consumes it; coordinated change in one repo.

## Migration Plan

1. Ship migrations (`account_events`, `index_cursors`) — additive only.
2. Deploy indexer; backfill from each chain's Bank deployment block (testnet history is small).
3. Switch `GetBalance` to live aggregation; add `GetAccountActivity`.
4. Point BFF resolvers at the new semantics; delete `relay_repo.get_balance`.
5. Refactor home-chain indexer onto the shared index; remove its in-memory cursor.

Rollback: resolvers revert to old RPCs; index tables are additive and can stay.

## Open Questions

- Should `GetRecentTransfers` be deleted in this change or deprecated one release later? (Default: delete — single consumer, single repo.)
- Backfill depth on testnets that may be reset — config flag `INDEX_FROM_BLOCK` per chain?
