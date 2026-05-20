CREATE TABLE connector_syncs (
    sync_id UUID PRIMARY KEY,
    connector_id UUID NOT NULL,
    status TEXT NOT NULL,
    discovered_count INT NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    completed_at TEXT NULL,
    error TEXT NULL
);

CREATE INDEX connector_syncs_by_connector ON connector_syncs (connector_id, started_at DESC);

CREATE TABLE connector_discovered_items (
    connector_id UUID NOT NULL,
    source_ref_key TEXT NOT NULL,
    title TEXT NOT NULL,
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    latest_sync_id UUID NOT NULL,
    PRIMARY KEY (connector_id, source_ref_key)
);
