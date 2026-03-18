-- treasury.rebalance_ops: records every cold-path CCIP rebalance operation.
--
-- op_id        — unique identifier: "{source_chain}-{dest_chain}-{timestamp_ms}"
-- status       — pending → submitted → completed | failed

CREATE TABLE treasury.rebalance_ops (
    id               BIGSERIAL PRIMARY KEY,
    op_id            TEXT UNIQUE NOT NULL,
    source_chain_id  BIGINT NOT NULL,
    dest_chain_id    BIGINT NOT NULL,
    amount_wei       NUMERIC(78, 0) NOT NULL,
    -- CCIP messageId (bytes32 hex) extracted from source tx logs, if available.
    ccip_message_id  TEXT,
    -- Transaction hash of the rebalance call on the source chain.
    source_tx_hash   TEXT,
    status           TEXT NOT NULL DEFAULT 'pending',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Efficient lookup of in-flight ops and recent history.
CREATE INDEX ON treasury.rebalance_ops (source_chain_id, dest_chain_id, status);
CREATE INDEX ON treasury.rebalance_ops (status, created_at DESC);
