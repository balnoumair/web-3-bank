-- treasury.reserve_ops: records every USDC reserve bridge operation.
--
-- Lifecycle:
--   pending    — row inserted before bridgeReserve tx confirmed on source
--   submitted  — bridgeReserve tx mined; bridge_message_id captured; awaiting Circle attestation
--   relayed    — bridgeIn submitted on destination; awaiting on-chain completion event
--   completed  — ReserveBridgeCompleted event observed on destination
--   failed     — stuck past timeout, or unrecoverable error mid-flow
--
-- bridge_type lets the same table back multiple adapters (CCTP today, Tempo custom later).

CREATE TABLE treasury.reserve_ops (
    id                     BIGSERIAL PRIMARY KEY,
    op_id                  TEXT UNIQUE NOT NULL,
    source_chain_id        BIGINT NOT NULL,
    dest_chain_id          BIGINT NOT NULL,
    amount_wei             NUMERIC(78, 0) NOT NULL,
    bridge_type            TEXT NOT NULL,
    -- bytes32 hex of messageId emitted by Bank's ReserveBridgeInitiated event.
    bridge_message_id      TEXT,
    source_tx_hash         TEXT,
    -- CCTP raw message bytes (0x-prefixed hex) returned by Circle's attestation API.
    cctp_message_bytes     TEXT,
    -- CCTP attestation signature (0x-prefixed hex) returned by Circle's attestation API.
    cctp_attestation       TEXT,
    -- Tx hash of bridgeIn call on the destination chain.
    dest_tx_hash           TEXT,
    status                 TEXT NOT NULL DEFAULT 'pending',
    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at           TIMESTAMPTZ
);

CREATE INDEX ON treasury.reserve_ops (source_chain_id, dest_chain_id, status);
CREATE INDEX ON treasury.reserve_ops (status, updated_at DESC);
-- Lookups by destination + messageId during ReserveBridgeCompleted scan.
CREATE INDEX ON treasury.reserve_ops (dest_chain_id, bridge_message_id);
