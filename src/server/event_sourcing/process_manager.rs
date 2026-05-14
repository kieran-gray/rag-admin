use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::server::application::AppError;

use super::aggregate::Aggregate;
use super::aggregate_repository::AggregateRepository;
use super::effect::{EffectLedger, EffectStatus, IdempotencyKey, PendingEffect};
use super::envelope::EventEnvelope;
use super::policy::PolicyContext;

const MAX_EFFECT_ATTEMPTS: i32 = 6;

#[async_trait]
pub trait EffectExecutor<R>: Send + Sync
where
    R: Send + Sync,
{
    async fn execute(&self, effect: &R) -> Result<(), AppError>;
}

pub struct ProcessManager<A, R>
where
    A: Aggregate,
    R: Serialize + DeserializeOwned + Send + Sync,
{
    repository: Arc<AggregateRepository<A>>,
    ledger: Arc<dyn EffectLedger<R>>,
    executor: Arc<dyn EffectExecutor<R>>,
    derive_effects: DeriveEffectsFn<A, R>,
    _phantom: PhantomData<R>,
}

pub type DeriveEffectsFn<A, R> =
    fn(envelope: &EventEnvelope<<A as Aggregate>::Event>, state: &A) -> Vec<PendingEffect<R>>;

impl<A, R> ProcessManager<A, R>
where
    A: Aggregate,
    R: Serialize + DeserializeOwned + Clone + Send + Sync,
    AppError: From<A::Error>,
{
    pub fn new(
        repository: Arc<AggregateRepository<A>>,
        ledger: Arc<dyn EffectLedger<R>>,
        executor: Arc<dyn EffectExecutor<R>>,
        derive_effects: DeriveEffectsFn<A, R>,
    ) -> Self {
        Self {
            repository,
            ledger,
            executor,
            derive_effects,
            _phantom: PhantomData,
        }
    }

    pub async fn enqueue_effects_for(
        &self,
        envelopes: &[EventEnvelope<A::Event>],
    ) -> Result<(), AppError> {
        if envelopes.is_empty() {
            return Ok(());
        }

        let mut by_stream: std::collections::BTreeMap<Uuid, Vec<&EventEnvelope<A::Event>>> =
            std::collections::BTreeMap::new();
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
            let mut pending: Vec<PendingEffect<R>> = Vec::new();
            for env in events {
                let _ctx = PolicyContext::new(env, state);
                pending.extend((self.derive_effects)(env, state));
            }
            if !pending.is_empty() {
                let effect_types: Vec<&'static str> =
                    pending.iter().map(|p| p.effect_type).collect();
                info!(
                    aggregate = A::aggregate_type(),
                    %stream_id,
                    count = pending.len(),
                    effects = ?effect_types,
                    "enqueued effects"
                );
                self.ledger.insert(A::aggregate_type(), &pending).await?;
            }
        }

        Ok(())
    }

    pub async fn dispatch_pending(&self) -> Result<(), AppError> {
        let pending = self
            .ledger
            .pending(A::aggregate_type(), MAX_EFFECT_ATTEMPTS)
            .await?;

        for record in pending {
            self.ledger.mark_dispatched(record.effect_id).await?;
            info!(
                aggregate = A::aggregate_type(),
                stream_id = %record.stream_id,
                effect_id = %record.effect_id,
                idempotency_key = %record.idempotency_key.as_str(),
                attempt = record.attempts + 1,
                "dispatching effect"
            );
            match self.executor.execute(&record.payload).await {
                Ok(()) => {
                    self.ledger.mark_completed(record.effect_id).await?;
                    info!(
                        aggregate = A::aggregate_type(),
                        stream_id = %record.stream_id,
                        effect_id = %record.effect_id,
                        "effect completed"
                    );
                }
                Err(e) => {
                    let next_attempts = record.attempts + 1;
                    warn!(
                        aggregate = A::aggregate_type(),
                        effect_id = %record.effect_id,
                        idempotency_key = %record.idempotency_key.as_str(),
                        attempt = next_attempts,
                        error = %e,
                        "effect dispatch failed"
                    );
                    if let Err(le) = self
                        .ledger
                        .mark_failed(record.effect_id, &e.to_string(), next_attempts)
                        .await
                    {
                        error!(
                            effect_id = %record.effect_id,
                            error = %le,
                            "failed to mark effect failed"
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

#[allow(dead_code)]
fn _trait_object_safety_assertions() {
    fn _is_object_safe<R: Send + Sync>(_: &dyn EffectExecutor<R>) {}
    fn _ledger_object_safe<R>(_: &dyn EffectLedger<R>)
    where
        R: Serialize + DeserializeOwned + Send + Sync,
    {
    }
    let _ = IdempotencyKey::new(Uuid::nil(), 0, "x");
    let _ = EffectStatus::Pending;
}
