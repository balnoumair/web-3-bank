CREATE TABLE treasury.relay_logs (
    id                BIGSERIAL    PRIMARY KEY,
    source_event_hash TEXT         NOT NULL UNIQUE,
    dest_tx_hash      TEXT,
    source_chain_id   BIGINT       NOT NULL,
    dest_chain_id     BIGINT       NOT NULL,
    recipient         TEXT         NOT NULL,
    amount_wei        NUMERIC(78)  NOT NULL,
    status            TEXT         NOT NULL DEFAULT 'pending',
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ  NOT NULL DEFAULT now()
);
