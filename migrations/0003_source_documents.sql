CREATE TABLE source_documents (
    document_id UUID PRIMARY KEY,
    document_type TEXT NOT NULL,
    source_ref JSONB NOT NULL,
    source_ref_key TEXT NOT NULL,
    latest_version_number INT NOT NULL,
    latest_content_hash TEXT NOT NULL,
    latest_metadata JSONB NOT NULL,
    latest_version_occurred_at TEXT NOT NULL,
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX source_documents_source_ref_idx ON source_documents USING GIN (source_ref);
CREATE INDEX source_documents_source_ref_key_idx ON source_documents (source_ref_key);

CREATE TABLE source_document_blobs (
    content_hash TEXT PRIMARY KEY,
    bytes BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
