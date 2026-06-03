## 1. Schema & migration

- [x] 1.1 Add migration `treasury.reserve_ledger_accounts` (account key e.g. `reserve:<chain_id>` / `in_transit` / `genesis`, kind, created_at) in the `treasury.*` schema.
- [x] 1.2 Add migration `treasury.reserve_ledger_transfers` (id, op_id, leg `initiation|completion|reversal`, debit_account, credit_account, amount_wei `NUMERIC(78,0)`, created_at) with a UNIQUE constraint on `(op_id, leg)` for idempotency.
- [x] 1.3 Add an index supporting per-account balance computation and recent-transfer lookups.
- [x] 1.4 ~Regenerate `.sqlx` query metadata~ — N/A: new ledger queries use sqlx's runtime functions (no DB needed at compile time), so the offline cache is untouched and existing macro entries stay valid. Convert to checked macros + `cargo sqlx prepare` later if a DB is available.

## 2. Domain model & port

- [x] 2.1 Define ledger domain types (`LedgerAccount`, `TransferLeg` enum, a balanced `LedgerTransfer` value object) in `src/domain/ledger.rs`, keeping construction pure and amounts as `U256`.
- [x] 2.2 Add a `ReserveLedgerRepository` driven-port trait in `src/domain/repository.rs`. Per the approved "atomic" decision the lifecycle recording lives in the reserve adapter's transaction (in-tx helper), so the port exposes the read/bootstrap surface — `account_balance(chain)`, `in_transit_balance()`, `seed_opening_balance`, `has_opening_balance` — rather than standalone `record_*` methods.
- [x] 2.3 Write a pure function that, given a reserve-op transition, returns the `LedgerTransfer` to post (debit/credit/amount), so the mapping is unit-testable without a DB.

## 3. Postgres adapter

- [x] 3.1 Implement `PgReserveLedgerRepository` (sqlx), inserting transfers with `ON CONFLICT (op_id, leg) DO NOTHING` for idempotency.
- [x] 3.2 Implement balance queries (sum of credits − debits per account).
- [x] 3.3 Expose a helper (`record_transfer_tx`) to record a ledger transfer **within an existing transaction**, so it commits atomically with the `reserve_ops` status update.

## 4. Hook ledger into the reserve-path lifecycle

- [x] 4.1 On `reserve_ops` → `submitted`, record the initiation transfer (`debit reserve:<src> → credit in_transit`) in the same DB transaction as the status update.
- [x] 4.2 On `reserve_ops` → `completed`, record the completion transfer (`debit in_transit → credit reserve:<dst>`) in the same DB transaction (guarded so it only fires when the initiation leg exists, keeping the books balanced).
- [x] 4.3 On `reserve_ops` → `failed` after an initiation transfer exists (and no completion), record the compensating reversal transfer so no value leaks in `in_transit`.

## 5. Bootstrap / opening balances

- [x] 5.1 On startup, if a chain's reserve account has no opening balance, read `reserveDepth()` once and post a `genesis → reserve:<chain>` opening transfer so the ledger starts reconciled.
- [x] 5.2 Make bootstrap idempotent (guarded by `has_opening_balance`; safe to run on every restart).

## 6. Reconciliation loop

- [x] 6.1 Add a periodic reconciliation task (config-driven cadence + tolerance) that reads `reserveDepth()` per active chain and compares to ledger reserve balances, accounting for in-flight value.
- [x] 6.2 On divergence beyond tolerance, raise an `AlertType::Mismatch` alert via the existing watcher alert path (hourly-bucketed key → at most one alert/chain/hour); never auto-correct the ledger.
- [x] 6.3 Add config keys (`reserve_recon_poll_secs`, `reserve_recon_tolerance_wei`) to `Config` and document them in `.env.example` (`.env` untouched).

## 7. Tests

- [x] 7.1 Unit-test the pure transition→transfer mapping (initiation, completion, reversal, opening). *(domain::ledger::tests — passing)*
- [x] 7.2 Unit-test invariants: in-transit returns to zero after completion; reversal drains in-transit on failure. *(pure: ledger::tests; DB-backed: reserve_ledger_repo::tests)*
- [x] 7.3 Test idempotency: re-recording the same `(op_id, leg)` does not double-count. *(reserve_ledger_repo::tests::recording_same_leg_twice_is_idempotent)*
- [x] 7.4 Test reconciliation decision: within-tolerance passes silently; beyond-tolerance reports the gap. *(reserve_path::tests::recon_* — passing)*

## 8. Verification

- [~] 8.1 `cargo build` (offline) and `cargo clippy --bins` pass clean; **pure** unit tests (10) pass. The 5 DB-backed `#[sqlx::test]` tests compile but were **not run here — no local Postgres available**. Run `cargo test` against a live DB to execute them.
- [x] 8.2 `openspec validate add-reserve-accounting-ledger --strict` passes (see below).
