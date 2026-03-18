-- Add a unique index on source_event_hash so the watcher can use
-- ON CONFLICT DO NOTHING for idempotent alert inserts.
-- source_event_hash stores the transferId (bytes32 hex) that links each
-- HotPathReleased event back to its HotPathInitiated source.
CREATE UNIQUE INDEX IF NOT EXISTS watcher_alerts_source_hash_idx
    ON treasury.watcher_alerts (source_event_hash);
