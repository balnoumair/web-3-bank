CREATE TABLE users.home_chain_audit (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tempo_address  TEXT        NOT NULL,
    previous_chain BIGINT,
    new_chain      BIGINT      NOT NULL,
    operator       TEXT        NOT NULL,
    reason         TEXT        NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ON users.home_chain_audit (tempo_address, created_at DESC);
