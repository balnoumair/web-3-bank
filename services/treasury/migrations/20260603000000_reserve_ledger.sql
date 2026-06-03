-- treasury.reserve_ledger_*: an internal double-entry mirror of USDC reserves.
--
-- This is a SECONDARY, observational ledger — NOT the source of truth. The chain
-- (Bank Contract `reserveDepth()`) remains authoritative. The ledger exists to:
--   1. model value that is mid-bridge (left source, not yet on destination) via a
--      shared `in_transit` account, so the books always balance; and
--   2. be reconciled against on-chain `reserveDepth()` to surface discrepancies.
--
-- Balances are DERIVED from `reserve_ledger_transfers` (sum of credits minus
-- debits per account). `reserve_ledger_accounts` is a lightweight registry of the
-- accounts that exist and their kind; transfers reference account keys as plain
-- text (no FK) to keep recording to a single insert, matching the rest of the
-- treasury schema's append-only style.

-- Account registry.
--   key examples: 'reserve:8453', 'in_transit', 'genesis'
--   kind:         'reserve' | 'in_transit' | 'genesis'
CREATE TABLE treasury.reserve_ledger_accounts (
    account_key  TEXT        PRIMARY KEY,
    kind         TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Immutable, append-only balanced transfers.
--   leg: 'opening'    — seeds a reserve account from genesis at bootstrap
--        'initiation' — bridge submitted: debit reserve:<src> → credit in_transit
--        'completion' — bridge completed: debit in_transit    → credit reserve:<dst>
--        'reversal'   — bridge failed after initiation: debit in_transit → credit reserve:<src>
--
-- UNIQUE (op_id, leg) makes every recording idempotent: re-processing a lifecycle
-- transition (retry, loop re-entry, restart) is a no-op insert.
CREATE TABLE treasury.reserve_ledger_transfers (
    id             BIGSERIAL PRIMARY KEY,
    op_id          TEXT NOT NULL,
    leg            TEXT NOT NULL,
    debit_account  TEXT NOT NULL,
    credit_account TEXT NOT NULL,
    amount_wei     NUMERIC(78, 0) NOT NULL CHECK (amount_wei >= 0),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (op_id, leg)
);

-- Balance computation scans transfers by account on both legs.
CREATE INDEX ON treasury.reserve_ledger_transfers (debit_account);
CREATE INDEX ON treasury.reserve_ledger_transfers (credit_account);
-- Recent-history / per-op lookups.
CREATE INDEX ON treasury.reserve_ledger_transfers (op_id);
