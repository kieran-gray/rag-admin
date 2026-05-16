DROP TABLE IF EXISTS pending_effects;

CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    queue TEXT NOT NULL,
    partition_key UUID,
    job_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    idempotency_key TEXT NOT NULL,
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 6,
    last_error TEXT,
    run_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    locked_until TIMESTAMPTZ,
    locked_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT jobs_idempotency_uk UNIQUE (queue, idempotency_key)
);

CREATE INDEX jobs_claim_idx ON jobs (queue, run_at, created_at);

CREATE INDEX jobs_partition_idx ON jobs (queue, partition_key)
WHERE
    partition_key IS NOT NULL;

CREATE TABLE dead_jobs (
    id UUID PRIMARY KEY,
    queue TEXT NOT NULL,
    partition_key UUID,
    job_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    idempotency_key TEXT NOT NULL,
    attempts INT NOT NULL,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    failed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX dead_jobs_queue_idx ON dead_jobs (queue, failed_at DESC);