CREATE TABLE connector_imports (
    import_id UUID PRIMARY KEY,
    connector_id UUID NOT NULL,
    status TEXT NOT NULL,
    total INT NOT NULL DEFAULT 0,
    imported INT NOT NULL DEFAULT 0,
    failed INT NOT NULL DEFAULT 0,
    index_after_import BOOLEAN NOT NULL DEFAULT FALSE,
    started_at TEXT NOT NULL,
    completed_at TEXT NULL,
    error TEXT NULL
);

CREATE INDEX connector_imports_by_connector ON connector_imports (connector_id, started_at DESC);
