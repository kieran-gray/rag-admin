CREATE TABLE chunk_sets (
    chunk_set_id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    document_version INT NOT NULL,
    chunking_config JSONB NOT NULL,
    pinned BOOLEAN NOT NULL DEFAULT FALSE,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    indexing_refs INTEGER NOT NULL DEFAULT 0,
    variant_result_refs INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX chunk_sets_document_id_idx ON chunk_sets (document_id);

CREATE INDEX chunk_sets_created_at_idx
    ON chunk_sets (created_at DESC, chunk_set_id DESC);

CREATE INDEX chunk_sets_pinned_idx
    ON chunk_sets (created_at DESC) WHERE pinned;

CREATE INDEX chunk_sets_indexed_idx
    ON chunk_sets (created_at DESC) WHERE indexing_refs > 0;

CREATE INDEX chunk_sets_eval_idx
    ON chunk_sets (created_at DESC) WHERE variant_result_refs > 0;

CREATE INDEX chunk_sets_unused_idx
    ON chunk_sets (created_at DESC)
    WHERE NOT pinned AND indexing_refs = 0 AND variant_result_refs = 0;

CREATE TABLE chunks (
    chunk_id UUID PRIMARY KEY,
    chunk_set_id UUID NOT NULL REFERENCES chunk_sets (chunk_set_id) ON DELETE CASCADE,
    sequence INT NOT NULL,
    heading TEXT NOT NULL,
    text TEXT NOT NULL,
    char_start INT NOT NULL,
    char_end INT NOT NULL
);

CREATE INDEX chunks_chunk_set_id_idx ON chunks (chunk_set_id, sequence);

CREATE TABLE embedding_sets (
    embedding_set_id UUID PRIMARY KEY,
    chunk_set_id UUID NOT NULL REFERENCES chunk_sets (chunk_set_id) ON DELETE CASCADE,
    embedding_model_id UUID NOT NULL,
    embedding_model_snapshot JSONB NOT NULL,
    dimensions INT NOT NULL,
    created_at TEXT NOT NULL,
    CONSTRAINT embedding_sets_chunk_model_unique UNIQUE (
        chunk_set_id,
        embedding_model_id
    )
);

CREATE TABLE indexings (
    indexing_id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    index_profile_id UUID NOT NULL,
    document_version INT NOT NULL,
    chunking_config JSONB NOT NULL,
    chunk_set_id UUID REFERENCES chunk_sets (chunk_set_id) ON DELETE RESTRICT,
    embedding_set_id UUID,
    status TEXT NOT NULL,
    failure_stage TEXT,
    attempts INT NOT NULL,
    removed BOOLEAN NOT NULL DEFAULT FALSE,
    auto_advance BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX indexings_document_id_idx ON indexings (document_id);

CREATE TABLE chunk_embeddings (
    chunk_id UUID NOT NULL REFERENCES chunks (chunk_id) ON DELETE CASCADE,
    embedding_set_id UUID NOT NULL REFERENCES embedding_sets (embedding_set_id) ON DELETE CASCADE,
    vec VECTOR NOT NULL,
    PRIMARY KEY (chunk_id, embedding_set_id)
);

CREATE INDEX chunk_embeddings_embedding_set_id_idx ON chunk_embeddings (embedding_set_id);

CREATE TABLE vector_index_records (
    index_name TEXT NOT NULL,
    id TEXT NOT NULL,
    vec VECTOR NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (index_name, id)
);

CREATE INDEX vector_index_records_index_name_idx ON vector_index_records (index_name);
