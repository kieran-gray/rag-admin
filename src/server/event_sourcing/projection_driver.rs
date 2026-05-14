use std::sync::Arc;
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::server::application::AppError;

use super::aggregate::Aggregate;
use super::checkpoint::{CheckpointRepository, CheckpointStatus, ProjectionCheckpoint};
use super::envelope::EventEnvelope;
use super::event_bus::EventBus;
use super::event_store::EventStore;
use super::process_manager::ProcessManager;
use super::projector::Projector;

const MAX_PROJECTOR_ERROR_COUNT: i64 = 5;
const POLL_HEARTBEAT: Duration = Duration::from_secs(2);
const BATCH_SIZE: i64 = 256;

pub struct ProjectionDriver<A, R>
where
    A: Aggregate,
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    event_store: Arc<dyn EventStore<A::Event>>,
    projectors: Vec<Arc<dyn Projector<A::Event>>>,
    checkpoint_repository: Arc<dyn CheckpointRepository>,
    event_bus: Arc<EventBus>,
    process_manager: Option<Arc<ProcessManager<A, R>>>,
    wakeup: Arc<Notify>,
}

impl<A, R> ProjectionDriver<A, R>
where
    A: Aggregate + 'static,
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
    AppError: From<A::Error>,
{
    pub fn new(
        event_store: Arc<dyn EventStore<A::Event>>,
        projectors: Vec<Arc<dyn Projector<A::Event>>>,
        checkpoint_repository: Arc<dyn CheckpointRepository>,
        event_bus: Arc<EventBus>,
        process_manager: Option<Arc<ProcessManager<A, R>>>,
        wakeup: Arc<Notify>,
    ) -> Self {
        Self {
            event_store,
            projectors,
            checkpoint_repository,
            event_bus,
            process_manager,
            wakeup,
        }
    }

    pub async fn tick(&self) -> Result<bool, AppError> {
        let mut any_work = false;

        for projector in &self.projectors {
            let checkpoint = self
                .checkpoint_repository
                .load(projector.name())
                .await?
                .unwrap_or_else(|| initial_checkpoint(projector.name()));

            if checkpoint.is_faulted(MAX_PROJECTOR_ERROR_COUNT) {
                warn!(projector = projector.name(), "skipping faulted projector");
                continue;
            }

            let envelopes = self
                .event_store
                .load_global_after(checkpoint.last_processed_log_position, BATCH_SIZE)
                .await?;

            if envelopes.is_empty() {
                continue;
            }
            any_work = true;

            match projector.project(&envelopes).await {
                Ok(()) => {
                    let last = envelopes.last().expect("non-empty");
                    let next = ProjectionCheckpoint {
                        projector_name: projector.name().to_string(),
                        last_processed_log_position: last.metadata.log_position,
                        status: CheckpointStatus::Healthy,
                        error_message: None,
                        error_count: 0,
                        updated_at: OffsetDateTime::now_utc(),
                    };
                    self.checkpoint_repository.upsert(&next).await?;
                    info!(
                        projector = projector.name(),
                        from = checkpoint.last_processed_log_position,
                        to = last.metadata.log_position,
                        count = envelopes.len(),
                        "projector advanced"
                    );
                    self.broadcast(&envelopes);
                    self.run_process_manager(&envelopes).await;
                }
                Err(e) => {
                    let next_count = checkpoint.error_count + 1;
                    error!(
                        projector = projector.name(),
                        attempt = next_count,
                        error = %e,
                        "projector failed batch"
                    );
                    let next = ProjectionCheckpoint {
                        projector_name: projector.name().to_string(),
                        last_processed_log_position: checkpoint.last_processed_log_position,
                        status: CheckpointStatus::Error,
                        error_message: Some(e.to_string()),
                        error_count: next_count,
                        updated_at: OffsetDateTime::now_utc(),
                    };
                    self.checkpoint_repository.upsert(&next).await?;
                }
            }
        }

        Ok(any_work)
    }

    fn broadcast(&self, envelopes: &[EventEnvelope<A::Event>]) {
        for env in envelopes {
            match env.to_published() {
                Ok(published) => self.event_bus.publish(Arc::new(published)),
                Err(e) => warn!(error = %e, "failed to serialize event for bus"),
            }
        }
    }

    async fn run_process_manager(&self, envelopes: &[EventEnvelope<A::Event>]) {
        let Some(pm) = &self.process_manager else {
            return;
        };
        if let Err(e) = pm.enqueue_effects_for(envelopes).await {
            error!(aggregate = A::aggregate_type(), error = %e, "process manager: enqueue failed");
            return;
        }
        if let Err(e) = pm.dispatch_pending().await {
            error!(aggregate = A::aggregate_type(), error = %e, "process manager: dispatch failed");
        }
    }

    pub async fn run(self: Arc<Self>) {
        info!(
            aggregate = A::aggregate_type(),
            projectors = self.projectors.len(),
            "projection driver started"
        );
        loop {
            loop {
                match self.tick().await {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => {
                        error!(aggregate = A::aggregate_type(), error = %e, "projection driver tick failed");
                        break;
                    }
                }
            }
            match timeout(POLL_HEARTBEAT, self.wakeup.notified()).await {
                Ok(()) => debug!(
                    aggregate = A::aggregate_type(),
                    "projection driver woken by notify"
                ),
                Err(_) => debug!(
                    aggregate = A::aggregate_type(),
                    "projection driver heartbeat tick"
                ),
            }
        }
    }
}

fn initial_checkpoint(name: &str) -> ProjectionCheckpoint {
    ProjectionCheckpoint {
        projector_name: name.to_string(),
        last_processed_log_position: 0,
        status: CheckpointStatus::Healthy,
        error_message: None,
        error_count: 0,
        updated_at: OffsetDateTime::now_utc(),
    }
}
