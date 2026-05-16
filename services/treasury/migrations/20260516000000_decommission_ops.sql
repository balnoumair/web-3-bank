-- treasury.decommission_ops: audit trail for governance-directed chain drains.

CREATE TABLE treasury.decommission_ops (
    id              BIGSERIAL PRIMARY KEY,
    op_id           TEXT UNIQUE,
    chain_id        BIGINT NOT NULL,
    holder_address  TEXT,
    amount          NUMERIC(78, 0) NOT NULL DEFAULT 0,
    src_message_id  TEXT,
    dst_chain_id    BIGINT NOT NULL,
    dst_tx_hash      TEXT,
    status          TEXT NOT NULL,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at    TIMESTAMPTZ,
    failure_reason  TEXT
);

CREATE INDEX ON treasury.decommission_ops (chain_id, dst_chain_id, status);
CREATE INDEX ON treasury.decommission_ops (holder_address, status);
CREATE INDEX ON treasury.decommission_ops (src_message_id);
