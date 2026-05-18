ALTER TABLE evaluation_runs
ADD COLUMN fingerprint JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE evaluation_runs
ALTER COLUMN fingerprint DROP DEFAULT;
