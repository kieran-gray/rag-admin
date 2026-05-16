DROP INDEX IF EXISTS pending_effects_pending_idx;

CREATE INDEX pending_effects_pending_idx ON pending_effects (
    aggregate_type,
    status,
    attempts,
    last_attempt_at
)
WHERE
    status IN ('pending', 'failed', 'dispatched');
