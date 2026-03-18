CREATE SCHEMA IF NOT EXISTS users;

CREATE TABLE users.users (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name TEXT        NOT NULL DEFAULT '',
    status       TEXT        NOT NULL DEFAULT 'active',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users.credentials (
    id             UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id        UUID    NOT NULL REFERENCES users.users(id),
    credential_id  BYTEA   NOT NULL UNIQUE,
    public_key     BYTEA   NOT NULL,
    tempo_address  TEXT    NOT NULL UNIQUE,
    revoked_at     TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
