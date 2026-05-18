use std::collections::BTreeMap;
use std::error::Error;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::task::JoinHandle;
use tokio::time::{interval as tokio_interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::aggregate::Aggregate;
use super::aggregate_repository::AggregateRepository;
use super::envelope::EventEnvelope;
use super::error::EsError;
use super::job_queue::{JobQueue, NewJob};
use super::policy::{HasPolicies, PolicyContext};

pub(crate) const LEASE: Duration = Duration::from_secs(30);
pub(crate) const HEARTBEAT: Duration = Duration::from_secs(10);

pub type EffectError = Box<dyn Error + Send + Sync + 'static>;

#[async_trait]
pub trait EffectExecutor<R>: Send + Sync
where
    R: Send + Sync,
{
    async fn execute(&self, effect: &R) -> Result<(), EffectError>;
}

pub struct ProcessManager<A, R>
where
    A: Aggregate,
    R: Serialize + DeserializeOwned + Send + Sync,
{
    repository: Arc<AggregateRepository<A>>,
    queue: Arc<dyn JobQueue<R>>,
    executor: Arc<dyn EffectExecutor<R>>,
    worker_id: String,
    _phantom: PhantomData<R>,
}

impl<A, R> ProcessManager<A, R>
where
    A: Aggregate + 'static,
    A::Event: HasPolicies<A, R>,
    R: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    pub fn new(
        repository: Arc<AggregateRepository<A>>,
        queue: Arc<dyn JobQueue<R>>,
        executor: Arc<dyn EffectExecutor<R>>,
    ) -> Self {
        Self {
            repository,
            queue,
            executor,
            worker_id: format!("{}-{}", A::aggregate_type(), Uuid::new_v4()),
            _phantom: PhantomData,
        }
    }

    pub async fn enqueue_effects_for(
        &self,
        envelopes: &[EventEnvelope<A::Event>],
    ) -> Result<(), EsError> {
        if envelopes.is_empty() {
            return Ok(());
        }

        let mut by_stream: BTreeMap<Uuid, Vec<&EventEnvelope<A::Event>>> = BTreeMap::new();
        for env in envelopes {
            by_stream
                .entry(env.metadata.stream_id)
                .or_default()
                .push(env);
        }

        for (stream_id, events) in by_stream {
            let Some(loaded) = self.repository.load(stream_id).await? else {
                debug!(%stream_id, "process manager: aggregate not found, skipping");
                continue;
            };
            let state = &loaded.aggregate;
            let mut pending: Vec<NewJob<R>> = Vec::new();
            for env in events {
                let ctx = PolicyContext::new(env, state);
                pending.extend(env.event.apply_policies(&ctx));
            }
            if !pending.is_empty() {
                let job_types: Vec<&'static str> = pending.iter().map(|p| p.job_type).collect();
                info!(
                    aggregate = A::aggregate_type(),
                    %stream_id,
                    count = pending.len(),
                    jobs = ?job_types,
                    "enqueued jobs"
                );
                self.queue.enqueue(A::aggregate_type(), &pending).await?;
            }
        }

        Ok(())
    }

    pub async fn claim_and_dispatch_one(&self) -> Result<bool, EsError> {
        let Some(job) = self
            .queue
            .claim(A::aggregate_type(), &self.worker_id, LEASE)
            .await?
        else {
            return Ok(false);
        };

        info!(
            aggregate = A::aggregate_type(),
            job_id = %job.id,
            partition = ?job.partition_key,
            idempotency_key = %job.idempotency_key.as_str(),
            attempt = job.attempts,
            "claimed job"
        );

        let _heartbeat = HeartbeatGuard::spawn(
            Arc::clone(&self.queue),
            job.id,
            self.worker_id.clone(),
            HEARTBEAT,
            LEASE,
        );

        let result = self.executor.execute(&job.payload).await;
        drop(_heartbeat);

        match result {
            Ok(()) => {
                self.queue.ack(job.id, &self.worker_id).await?;
                info!(
                    aggregate = A::aggregate_type(),
                    job_id = %job.id,
                    "job completed"
                );
            }
            Err(e) => {
                let attempts = job.attempts;
                if attempts >= job.max_attempts {
                    error!(
                        aggregate = A::aggregate_type(),
                        job_id = %job.id,
                        idempotency_key = %job.idempotency_key.as_str(),
                        attempt = attempts,
                        error = %e,
                        "job exhausted retries, moving to dead_jobs"
                    );
                    self.queue.dead_letter(job.id, &e.to_string()).await?;
                } else {
                    let delay = backoff(attempts);
                    warn!(
                        aggregate = A::aggregate_type(),
                        job_id = %job.id,
                        idempotency_key = %job.idempotency_key.as_str(),
                        attempt = attempts,
                        backoff_secs = delay.as_secs(),
                        error = %e,
                        "job failed, retrying"
                    );
                    self.queue
                        .nack(job.id, &self.worker_id, &e.to_string(), delay)
                        .await?;
                }
            }
        }
        Ok(true)
    }
}

fn backoff(attempts: i32) -> Duration {
    const BASE_SECS: u64 = 2;
    const MAX_SECS: u64 = 300;
    let exp = (attempts.clamp(1, 8) as u32) - 1;
    Duration::from_secs((BASE_SECS << exp).min(MAX_SECS))
}

struct HeartbeatGuard {
    handle: JoinHandle<()>,
}

impl HeartbeatGuard {
    fn spawn<R>(
        queue: Arc<dyn JobQueue<R>>,
        job_id: Uuid,
        worker_id: String,
        interval: Duration,
        lease: Duration,
    ) -> Self
    where
        R: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let handle = tokio::spawn(async move {
            let mut ticker = tokio_interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                match queue.heartbeat(job_id, &worker_id, lease).await {
                    Ok(true) => {}
                    Ok(false) => {
                        warn!(%job_id, "heartbeat lost lease");
                        return;
                    }
                    Err(e) => {
                        error!(%job_id, error = %e, "heartbeat error");
                    }
                }
            }
        });
        Self { handle }
    }
}

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[allow(dead_code)]
fn _trait_object_safety_assertions() {
    fn _is_object_safe<R: Send + Sync>(_: &dyn EffectExecutor<R>) {}
    fn _queue_object_safe<R>(_: &dyn JobQueue<R>)
    where
        R: Serialize + DeserializeOwned + Send + Sync,
    {
    }
}
