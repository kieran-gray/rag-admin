CREATE TABLE events (
    id BIGSERIAL PRIMARY KEY,
    stream_id UUID NOT NULL,
    aggregate_type TEXT NOT NULL,
    position BIGINT NOT NULL,
    event_type TEXT NOT NULL,
    event_data JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT events_stream_position_unique UNIQUE (stream_id, position)
);

CREATE INDEX events_stream_id_idx ON events (stream_id, position);

CREATE INDEX events_aggregate_type_id_idx ON events (aggregate_type, id);

CREATE TABLE aggregate_snapshots (
    stream_id UUID PRIMARY KEY,
    aggregate_type TEXT NOT NULL,
    version BIGINT NOT NULL,
    snapshot JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE projection_checkpoints (
    projector_name TEXT PRIMARY KEY,
    last_processed_log_position BIGINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'healthy',
    error_message TEXT,
    error_count BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION notify_events_appended() RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('events_appended', NEW.aggregate_type);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER events_appended_trigger
    AFTER INSERT ON events
    FOR EACH ROW EXECUTE FUNCTION notify_events_appended();

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

CREATE TABLE kv_store (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
