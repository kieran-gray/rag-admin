CREATE TABLE reranker_models (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    model TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (kind, model)
);

ALTER TABLE retrieval_profiles
    ADD CONSTRAINT retrieval_profiles_reranker_fk
    FOREIGN KEY (reranker_model_id)
    REFERENCES reranker_models (id)
    ON DELETE RESTRICT;
