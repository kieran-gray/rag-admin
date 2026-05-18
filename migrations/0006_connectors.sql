CREATE TABLE connectors (
    connector_id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    config JSONB NOT NULL,
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX connectors_active_kind_idx ON connectors (kind)
WHERE
    NOT deleted;
