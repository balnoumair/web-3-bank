CREATE TABLE treasury.pool_snapshots (
    id          BIGSERIAL    PRIMARY KEY,
    chain_id    BIGINT       NOT NULL,
    depth_wei   NUMERIC(78)  NOT NULL,
    recorded_at TIMESTAMPTZ  NOT NULL DEFAULT now()
);
