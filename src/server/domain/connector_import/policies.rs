use event_sourcing::job_queue::{IdempotencyKey, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};

use super::aggregate::ConnectorImport;
use super::effects::{ConnectorImportEffect, RunConnectorImportEffect};
use super::events::ConnectorImportEvent;

const RUN: &str = "run_connector_import";

impl HasPolicies<ConnectorImport, ConnectorImportEffect> for ConnectorImportEvent {
    fn policies() -> &'static [PolicyFn<Self, ConnectorImport, ConnectorImportEffect>] {
        &[run_on_started]
    }
}

fn run_on_started(
    event: &ConnectorImportEvent,
    ctx: &PolicyContext<'_, ConnectorImport, ConnectorImportEvent>,
) -> Vec<NewJob<ConnectorImportEffect>> {
    match event {
        ConnectorImportEvent::ConnectorImportStarted(_) => {
            let import_id = ctx.state.import_id;
            let log_position = ctx.envelope.metadata.log_position;
            vec![NewJob {
                partition_key: import_id,
                job_type: RUN,
                idempotency_key: IdempotencyKey::new(import_id, log_position, RUN),
                payload: ConnectorImportEffect::Run(RunConnectorImportEffect { import_id }),
            }]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use event_sourcing::envelope::{EventEnvelope, EventMetadata};
    use uuid::Uuid;

    use crate::server::domain::connector_import::aggregate::ImportStatus;
    use crate::server::domain::connector_import::events::{
        ConnectorImportCompleted, ConnectorImportStarted,
    };

    fn state(import_id: Uuid) -> ConnectorImport {
        ConnectorImport {
            import_id,
            connector_id: Uuid::new_v4(),
            index_after_import: true,
            items: Vec::new(),
            status: ImportStatus::Running,
            started_at: "2024-01-01T00:00:00Z".into(),
        }
    }

    fn envelope(stream_id: Uuid, log_position: i64) -> EventEnvelope<ConnectorImportEvent> {
        EventEnvelope {
            event: ConnectorImportEvent::ConnectorImportStarted(ConnectorImportStarted {
                import_id: stream_id,
                connector_id: Uuid::new_v4(),
                source_ref_keys: vec!["url:a".into()],
                index_after_import: true,
                occurred_at: "2024-01-01T00:00:00Z".into(),
            }),
            metadata: EventMetadata {
                stream_id,
                aggregate_type: "connector_import".into(),
                sequence: 1,
                log_position,
                event_type: "ConnectorImportStarted".into(),
                occurred_at: "2024-01-01T00:00:00Z".into(),
            },
        }
    }

    #[test]
    fn started_enqueues_single_run_effect() {
        let s = state(Uuid::new_v4());
        let env = envelope(s.import_id, 7);
        let ctx = PolicyContext::new(&env, &s);

        let started = ConnectorImportEvent::ConnectorImportStarted(ConnectorImportStarted {
            import_id: s.import_id,
            connector_id: s.connector_id,
            source_ref_keys: vec!["url:a".into()],
            index_after_import: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs = run_on_started(&started, &ctx);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_type, "run_connector_import");
        assert_eq!(jobs[0].partition_key, s.import_id);
        assert!(matches!(jobs[0].payload, ConnectorImportEffect::Run(_)));
    }

    #[test]
    fn other_events_enqueue_nothing() {
        let s = state(Uuid::new_v4());
        let env = envelope(s.import_id, 8);
        let ctx = PolicyContext::new(&env, &s);

        let completed = ConnectorImportEvent::ConnectorImportCompleted(ConnectorImportCompleted {
            import_id: s.import_id,
            imported: 1,
            failed: 0,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        assert!(run_on_started(&completed, &ctx).is_empty());
    }

    #[test]
    fn idempotency_key_uses_log_position() {
        let s = state(Uuid::new_v4());
        let env_a = envelope(s.import_id, 1);
        let env_b = envelope(s.import_id, 2);
        let started = ConnectorImportEvent::ConnectorImportStarted(ConnectorImportStarted {
            import_id: s.import_id,
            connector_id: s.connector_id,
            source_ref_keys: vec!["url:a".into()],
            index_after_import: true,
            occurred_at: "2024-01-01T00:00:00Z".into(),
        });
        let jobs_a = run_on_started(&started, &PolicyContext::new(&env_a, &s));
        let jobs_b = run_on_started(&started, &PolicyContext::new(&env_b, &s));
        assert_ne!(
            jobs_a[0].idempotency_key.as_str(),
            jobs_b[0].idempotency_key.as_str()
        );
    }
}
