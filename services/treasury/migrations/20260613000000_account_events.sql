-- Persistent index of on-chain account-affecting events per chain.
CREATE TABLE treasury.account_events (
    id              BIGSERIAL    PRIMARY KEY,
    chain_id        BIGINT       NOT NULL,
    tx_hash         TEXT         NOT NULL,
    log_index       INTEGER      NOT NULL,
    event_kind      TEXT         NOT NULL,
    address_from    TEXT,
    address_to      TEXT,
    amount_wei      NUMERIC(78)  NOT NULL,
    block_number    BIGINT       NOT NULL,
    block_time      TIMESTAMPTZ,
    correlation     TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
    UNIQUE (chain_id, tx_hash, log_index)
);

CREATE INDEX account_events_address_from_idx
    ON treasury.account_events (address_from)
    WHERE address_from IS NOT NULL;

CREATE INDEX account_events_address_to_idx
    ON treasury.account_events (address_to)
    WHERE address_to IS NOT NULL;

CREATE INDEX account_events_block_time_idx
    ON treasury.account_events (block_time DESC NULLS LAST);

-- Per-chain block cursor for the account event indexer.
CREATE TABLE treasury.index_cursors (
    chain_id    BIGINT       PRIMARY KEY,
    last_block  BIGINT       NOT NULL,
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);
