ALTER TABLE source_documents
    ADD COLUMN source_ref_key TEXT;

UPDATE source_documents
SET source_ref_key = CASE source_ref->>'type'
    WHEN 'Upload' THEN 'upload:' || (source_ref->'data'->>'upload_id')
    WHEN 'Url' THEN 'url:' || (source_ref->'data'->>'url')
END;

ALTER TABLE source_documents
    ALTER COLUMN source_ref_key SET NOT NULL;

CREATE INDEX source_documents_source_ref_key_idx
    ON source_documents (source_ref_key);

DROP TABLE connector_imports;
