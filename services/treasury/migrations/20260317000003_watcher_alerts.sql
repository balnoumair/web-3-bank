CREATE TABLE treasury.watcher_alerts (
    id                BIGSERIAL   PRIMARY KEY,
    source_event_hash TEXT        NOT NULL,
    alert_type        TEXT        NOT NULL,
    detail            TEXT,
    resolved          BOOLEAN     NOT NULL DEFAULT false,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
