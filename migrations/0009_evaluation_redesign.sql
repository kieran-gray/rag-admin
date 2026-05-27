DROP TABLE IF EXISTS retrieval_traces CASCADE;
DROP TABLE IF EXISTS evaluation_variant_results CASCADE;
DROP TABLE IF EXISTS evaluation_run_list_view CASCADE;
DROP TABLE IF EXISTS evaluation_runs CASCADE;
DROP TABLE IF EXISTS evaluation_references CASCADE;
DROP TABLE IF EXISTS evaluation_questions CASCADE;
DROP TABLE IF EXISTS evaluation_datasets CASCADE;

DELETE FROM aggregate_snapshots WHERE aggregate_type IN ('evaluation_dataset', 'evaluation_run');
DELETE FROM events WHERE aggregate_type IN ('evaluation_dataset', 'evaluation_run');
DELETE FROM projection_checkpoints
    WHERE projector_name IN (
        'evaluation_dataset_projector',
        'evaluation_run_projector',
        'evaluation_run_chunk_set_ref_projector'
    );

CREATE TABLE evaluation_datasets (
    dataset_id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    document_title TEXT,
    document_version INT NOT NULL,
    content_hash TEXT NOT NULL,
    label TEXT NOT NULL,
    target_question_count INT NOT NULL,
    generation_model_id UUID NOT NULL,
    generation_model TEXT NOT NULL,
    duplicate_similarity_threshold_milli INT NOT NULL,
    embedding_model_id UUID NOT NULL,
    status TEXT NOT NULL,
    map_ready BOOLEAN NOT NULL DEFAULT FALSE,
    map_failure_reason TEXT,
    question_count INT NOT NULL DEFAULT 0,
    rejection_count INT NOT NULL DEFAULT 0,
    run_count INTEGER NOT NULL DEFAULT 0,
    failure_reason TEXT,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX evaluation_datasets_document_id_idx ON evaluation_datasets (document_id);
CREATE INDEX evaluation_datasets_deleted_at_idx ON evaluation_datasets (deleted_at)
    WHERE deleted_at IS NULL;

CREATE TABLE evaluation_questions (
    dataset_id UUID NOT NULL REFERENCES evaluation_datasets (dataset_id) ON DELETE CASCADE,
    sequence INT NOT NULL,
    question TEXT NOT NULL,
    embedding JSONB,
    operation TEXT NOT NULL,
    evidence_kind TEXT NOT NULL,
    role TEXT NOT NULL,
    evidence_refs JSONB NOT NULL,
    PRIMARY KEY (dataset_id, sequence)
);

CREATE INDEX idx_eval_questions_axes
    ON evaluation_questions (dataset_id, operation, evidence_kind);

CREATE TABLE evaluation_references (
    dataset_id UUID NOT NULL,
    question_sequence INT NOT NULL,
    sequence INT NOT NULL,
    document_id UUID NOT NULL,
    content TEXT NOT NULL,
    char_start INT NOT NULL,
    char_end INT NOT NULL,
    embedding JSONB,
    PRIMARY KEY (dataset_id, question_sequence, sequence),
    FOREIGN KEY (dataset_id, question_sequence)
        REFERENCES evaluation_questions (dataset_id, sequence) ON DELETE CASCADE
);

CREATE TABLE evaluation_runs (
    run_id UUID PRIMARY KEY,
    dataset_id UUID NOT NULL REFERENCES evaluation_datasets (dataset_id),
    index_profile_id UUID NOT NULL,
    retrieval_profile_id UUID,
    document_id UUID NOT NULL,
    document_version INT NOT NULL,
    variants JSONB NOT NULL,
    options JSONB NOT NULL,
    optimization JSONB,
    status TEXT NOT NULL,
    variants_count INT NOT NULL,
    variants_prepared INT NOT NULL,
    variants_scored INT NOT NULL,
    failure_reason TEXT,
    scoring_recall_weight REAL NOT NULL,
    scoring_iou_weight REAL NOT NULL,
    scoring_precision_weight REAL NOT NULL,
    scoring_precision_omega_weight REAL NOT NULL,
    fingerprint JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX evaluation_runs_document_id_idx ON evaluation_runs (document_id);
CREATE INDEX evaluation_runs_dataset_id_idx ON evaluation_runs (dataset_id);

CREATE TABLE evaluation_variant_results (
    run_id UUID NOT NULL REFERENCES evaluation_runs (run_id) ON DELETE CASCADE,
    variant_label TEXT NOT NULL,
    split TEXT NOT NULL CHECK (split IN ('full', 'tuning', 'validation', 'holdout')),
    top_k INTEGER NOT NULL,
    min_score_milli INTEGER NOT NULL,
    variant_config JSONB NOT NULL,
    options JSONB NOT NULL,
    recall_mean REAL NOT NULL,
    recall_std REAL NOT NULL,
    precision_mean REAL NOT NULL,
    precision_std REAL NOT NULL,
    iou_mean REAL NOT NULL,
    iou_std REAL NOT NULL,
    precision_omega_mean REAL NOT NULL,
    precision_omega_std REAL NOT NULL,
    recall_ci_low REAL NOT NULL DEFAULT 0,
    recall_ci_high REAL NOT NULL DEFAULT 0,
    precision_ci_low REAL NOT NULL DEFAULT 0,
    precision_ci_high REAL NOT NULL DEFAULT 0,
    iou_ci_low REAL NOT NULL DEFAULT 0,
    iou_ci_high REAL NOT NULL DEFAULT 0,
    precision_omega_ci_low REAL NOT NULL DEFAULT 0,
    precision_omega_ci_high REAL NOT NULL DEFAULT 0,
    composite_ci_low REAL NOT NULL DEFAULT 0,
    composite_ci_high REAL NOT NULL DEFAULT 0,
    judge_score REAL,
    chunk_set_id UUID NOT NULL REFERENCES chunk_sets (chunk_set_id) ON DELETE RESTRICT,
    embedding_set_id UUID NOT NULL,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    average_chunk_tokens INTEGER NOT NULL DEFAULT 0,
    average_retrieved_tokens INTEGER NOT NULL DEFAULT 0,
    selected BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (run_id, variant_label, split, top_k, min_score_milli)
);

CREATE TABLE retrieval_traces (
    run_id UUID NOT NULL,
    variant_label TEXT NOT NULL,
    split TEXT NOT NULL CHECK (split IN ('full', 'tuning', 'validation', 'holdout')),
    top_k INTEGER NOT NULL,
    min_score_milli INTEGER NOT NULL,
    question_sequence INT NOT NULL,
    operation TEXT NOT NULL,
    evidence_kind TEXT NOT NULL,
    retrieved_chunk_ids JSONB NOT NULL,
    scores JSONB NOT NULL,
    recall REAL NOT NULL,
    precision REAL NOT NULL,
    iou REAL NOT NULL,
    PRIMARY KEY (
        run_id,
        variant_label,
        split,
        top_k,
        min_score_milli,
        question_sequence
    ),
    CONSTRAINT retrieval_traces_variant_fkey
        FOREIGN KEY (run_id, variant_label, split, top_k, min_score_milli)
        REFERENCES evaluation_variant_results (run_id, variant_label, split, top_k, min_score_milli)
        ON DELETE CASCADE
);

CREATE TABLE evaluation_run_list_view (
    run_id UUID PRIMARY KEY,
    dataset_id UUID NOT NULL,
    document_id UUID NOT NULL,
    optimization JSONB,
    kind TEXT NOT NULL CHECK (kind IN ('optimization', 'manual')),
    status TEXT NOT NULL,
    failure_reason TEXT,
    variant_count INTEGER NOT NULL,
    variants_scored INTEGER NOT NULL,
    best_variant JSONB,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX evaluation_run_list_view_created_at_idx
    ON evaluation_run_list_view (created_at DESC, run_id DESC);
CREATE INDEX evaluation_run_list_view_dataset_idx
    ON evaluation_run_list_view (dataset_id, created_at DESC);
CREATE INDEX evaluation_run_list_view_document_idx
    ON evaluation_run_list_view (document_id, created_at DESC);
CREATE INDEX evaluation_run_list_view_status_idx
    ON evaluation_run_list_view (status);
CREATE INDEX evaluation_run_list_view_kind_idx
    ON evaluation_run_list_view (kind);
