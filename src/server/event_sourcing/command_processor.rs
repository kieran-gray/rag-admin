use std::sync::Arc;

use tracing::{debug, info};
use uuid::Uuid;

use crate::server::application::AppError;

use super::aggregate::Aggregate;
use super::aggregate_repository::AggregateRepository;
use super::envelope::EventEnvelope;

const SNAPSHOT_AFTER_EVENTS: usize = 16;

pub struct CommandProcessor<A>
where
    A: Aggregate,
{
    repository: Arc<AggregateRepository<A>>,
}

impl<A> CommandProcessor<A>
where
    A: Aggregate,
    AppError: From<A::Error>,
{
    pub fn new(repository: Arc<AggregateRepository<A>>) -> Self {
        Self { repository }
    }

    pub async fn handle(
        &self,
        stream_id: Uuid,
        command: A::Command,
    ) -> Result<Vec<EventEnvelope<A::Event>>, AppError> {
        let loaded = self.repository.load(stream_id).await?;
        let (state_ref, expected_version, prior_tail) = match &loaded {
            Some(l) => (
                Some(&l.aggregate),
                l.version as usize,
                l.new_events_since_snapshot,
            ),
            None => (None, 0, 0),
        };

        let new_events = A::handle_command(state_ref, command).map_err(AppError::from)?;
        if new_events.is_empty() {
            debug!(
                aggregate = A::aggregate_type(),
                %stream_id,
                "command produced no events"
            );
            return Ok(vec![]);
        }

        let appended = self
            .repository
            .event_store()
            .append(stream_id, expected_version, &new_events)
            .await?;

        let event_types: Vec<&str> = appended
            .iter()
            .map(|e| e.metadata.event_type.as_str())
            .collect();
        info!(
            aggregate = A::aggregate_type(),
            %stream_id,
            count = appended.len(),
            events = ?event_types,
            "appended events"
        );

        let next_state = match loaded {
            Some(l) => {
                let mut state = l.aggregate;
                for env in &appended {
                    state.apply(&env.event);
                }
                state
            }
            None => A::from_events(&new_events).ok_or_else(|| {
                AppError::Internal(format!(
                    "aggregate {} produced events without a valid creation event",
                    A::aggregate_type()
                ))
            })?,
        };

        let new_version = appended
            .last()
            .map(|e| e.metadata.sequence)
            .unwrap_or(expected_version as i64);

        if prior_tail + appended.len() >= SNAPSHOT_AFTER_EVENTS {
            self.repository
                .save_snapshot(stream_id, new_version, &next_state)
                .await?;
            debug!(
                aggregate = A::aggregate_type(),
                %stream_id,
                version = new_version,
                "refreshed snapshot"
            );
        }

        // `next_state` is dropped here intentionally; callers that need it can
        // re-load through the repository (which will read the snapshot).
        let _ = next_state;

        Ok(appended)
    }
}
