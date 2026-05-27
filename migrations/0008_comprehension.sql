CREATE TABLE document_maps (
    map_id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    document_version INT NOT NULL,
    content_hash TEXT NOT NULL,
    chunk_set_id UUID NOT NULL,
    chunk_count INT NOT NULL,
    section_size INT NOT NULL,
    generation_model_id UUID NOT NULL,
    status TEXT NOT NULL,
    failure_reason TEXT,
    suggested_roles JSONB NOT NULL DEFAULT '[]'::jsonb,
    carried_thread_summary TEXT NOT NULL DEFAULT '',
    extracted_chunks INT[] NOT NULL DEFAULT ARRAY[]::INT[],
    synthesized_sections INT[] NOT NULL DEFAULT ARRAY[]::INT[],
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (document_id, document_version)
);

CREATE INDEX document_maps_document_id_idx ON document_maps (document_id);
CREATE INDEX document_maps_status_idx ON document_maps (status);

CREATE TABLE map_observations (
    map_id UUID NOT NULL REFERENCES document_maps (map_id) ON DELETE CASCADE,
    observation_id UUID NOT NULL,
    chunk_sequence INT NOT NULL,
    kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    spans JSONB NOT NULL,
    PRIMARY KEY (map_id, observation_id)
);

CREATE INDEX map_observations_chunk_idx ON map_observations (map_id, chunk_sequence);

CREATE TABLE map_threads (
    map_id UUID NOT NULL REFERENCES document_maps (map_id) ON DELETE CASCADE,
    thread_id UUID NOT NULL,
    section_sequence INT NOT NULL,
    kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    evidence JSONB NOT NULL,
    spans JSONB NOT NULL,
    PRIMARY KEY (map_id, thread_id)
);

CREATE INDEX map_threads_section_idx ON map_threads (map_id, section_sequence);

CREATE TABLE map_insights (
    map_id UUID NOT NULL REFERENCES document_maps (map_id) ON DELETE CASCADE,
    insight_id UUID NOT NULL,
    kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    evidence JSONB NOT NULL,
    spans JSONB NOT NULL,
    PRIMARY KEY (map_id, insight_id)
);
