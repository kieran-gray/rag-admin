CREATE TABLE connector_imports (
    connector_id UUID NOT NULL,
    document_id UUID NOT NULL,
    source_ref_key TEXT NOT NULL,
    first_imported_at TEXT NOT NULL,
    last_imported_at TEXT NOT NULL,
    latest_sync_id UUID NULL,
    PRIMARY KEY (connector_id, document_id)
);

CREATE INDEX connector_imports_by_document ON connector_imports (document_id);
CREATE INDEX connector_imports_by_connector ON connector_imports (connector_id);
