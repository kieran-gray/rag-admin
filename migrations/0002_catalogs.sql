CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE embedding_models (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (kind, model),
    CONSTRAINT embedding_models_id_dimensions_uk UNIQUE (id, dimensions)
);

CREATE TABLE generation_models (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    model TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (kind, model)
);

CREATE TABLE vector_indexes (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL UNIQUE,
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT vector_indexes_id_dimensions_uk UNIQUE (id, dimensions)
);

CREATE TABLE index_profiles (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    embedding_model_id UUID NOT NULL,
    vector_index_id UUID NOT NULL,
    dimensions INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT index_profiles_dimensions_positive CHECK (dimensions > 0),
    CONSTRAINT index_profiles_embedding_model_fk FOREIGN KEY (
        embedding_model_id,
        dimensions
    ) REFERENCES embedding_models (id, dimensions) ON DELETE RESTRICT,
    CONSTRAINT index_profiles_vector_index_fk FOREIGN KEY (
        vector_index_id,
        dimensions
    ) REFERENCES vector_indexes (id, dimensions) ON DELETE RESTRICT
);

CREATE TABLE retrieval_profiles (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    index_profile_id UUID NOT NULL REFERENCES index_profiles (id) ON DELETE RESTRICT,
    generation_model_id UUID NOT NULL REFERENCES generation_models (id) ON DELETE RESTRICT,
    reranker_model_id UUID,
    default_top_k INTEGER NOT NULL,
    default_min_score_milli INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT retrieval_profiles_top_k_positive CHECK (default_top_k > 0),
    CONSTRAINT retrieval_profiles_min_score_bounded CHECK (default_min_score_milli BETWEEN 0 AND 1000)
);

CREATE TABLE chunking_configurations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    config JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sweep_templates (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    members JSONB NOT NULL DEFAULT '[]',
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX sweep_templates_one_default ON sweep_templates (is_default)
WHERE
    is_default;

CREATE TABLE configuration_defaults (
    id INT PRIMARY KEY DEFAULT 1,
    chunking_configuration_id UUID,
    index_profile_id UUID,
    retrieval_profile_id UUID,
    sweep_template_id UUID,
    CONSTRAINT configuration_defaults_singleton CHECK (id = 1)
);
