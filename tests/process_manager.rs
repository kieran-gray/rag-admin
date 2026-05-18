#![cfg(feature = "ssr")]

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::aggregate::Aggregate;
use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::envelope::{EventEnvelope, EventMetadata};
use event_sourcing::event_store::EventStore;
use event_sourcing::job_queue::{IdempotencyKey, JobQueue, NewJob};
use event_sourcing::policy::{HasPolicies, PolicyContext, PolicyFn};
use event_sourcing::process_manager::{EffectError, EffectExecutor, ProcessManager};
use rag_admin::server::infrastructure::event_sourcing::{
    PostgresAggregateSnapshotStore, PostgresEventStore, PostgresJobQueue,
};

const AGGREGATE: &str = "process_manager_test";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CounterAggregate {
    count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
enum CounterEvent {
    Incremented { by: i64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct CounterEffect {
    by: i64,
}

#[derive(Debug, thiserror::Error)]
#[error("never")]
struct CounterError;

impl Aggregate for CounterAggregate {
    type Event = CounterEvent;
    type Command = ();
    type Error = CounterError;

    fn aggregate_type() -> &'static str {
        AGGREGATE
    }

    fn apply(&mut self, event: &Self::Event) {
        let CounterEvent::Incremented { by } = event;
        self.count += by;
    }

    fn handle_command(
        _state: Option<&Self>,
        _command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        Ok(Vec::new())
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        if events.is_empty() {
            return None;
        }
        let mut state = Self { count: 0 };
        for event in events {
            state.apply(event);
        }
        Some(state)
    }
}

fn emit_effect(
    event: &CounterEvent,
    ctx: &PolicyContext<'_, CounterAggregate, CounterEvent>,
) -> Vec<NewJob<CounterEffect>> {
    let CounterEvent::Incremented { by } = event;
    let log_position = ctx.envelope.metadata.log_position;
    vec![NewJob {
        partition_key: ctx.envelope.metadata.stream_id,
        job_type: "counter_effect",
        idempotency_key: IdempotencyKey::new(
            ctx.envelope.metadata.stream_id,
            log_position,
            "counter_effect",
        ),
        payload: CounterEffect { by: *by },
    }]
}

impl HasPolicies<CounterAggregate, CounterEffect> for CounterEvent {
    fn policies() -> &'static [PolicyFn<Self, CounterAggregate, CounterEffect>] {
        &[emit_effect]
    }
}

struct RecordingExecutor {
    seen: Arc<Mutex<Vec<CounterEffect>>>,
}

#[async_trait]
impl EffectExecutor<CounterEffect> for RecordingExecutor {
    async fn execute(&self, effect: &CounterEffect) -> Result<(), EffectError> {
        self.seen.lock().expect("lock").push(effect.clone());
        Ok(())
    }
}

async fn append_event(
    pool: &sqlx::PgPool,
    stream: Uuid,
    expected: usize,
    by: i64,
) -> EventEnvelope<CounterEvent> {
    let store = PostgresEventStore::<CounterEvent>::new(pool.clone(), AGGREGATE);
    let envs = store
        .append(stream, expected, &[CounterEvent::Incremented { by }])
        .await
        .expect("append event");
    envs.into_iter().next().expect("one envelope")
}

fn build_process_manager(
    pool: sqlx::PgPool,
) -> (
    ProcessManager<CounterAggregate, CounterEffect>,
    Arc<Mutex<Vec<CounterEffect>>>,
    Arc<PostgresJobQueue<CounterEffect>>,
) {
    let event_store: Arc<dyn EventStore<CounterEvent>> =
        Arc::new(PostgresEventStore::new(pool.clone(), AGGREGATE));
    let snapshot_store = Arc::new(PostgresAggregateSnapshotStore::<CounterAggregate>::new(
        pool.clone(),
    ));
    let repository = Arc::new(AggregateRepository::new(event_store, snapshot_store));

    let queue = Arc::new(PostgresJobQueue::<CounterEffect>::new(pool));
    let queue_dyn: Arc<dyn JobQueue<CounterEffect>> = queue.clone();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let executor: Arc<dyn EffectExecutor<CounterEffect>> = Arc::new(RecordingExecutor {
        seen: Arc::clone(&seen),
    });
    let pm = ProcessManager::new(repository, queue_dyn, executor);
    (pm, seen, queue)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_emits_one_job_per_event_and_executor_runs_them() {
    let harness = common::start().await;
    let stream = Uuid::now_v7();
    let env = append_event(&harness.pool, stream, 0, 5).await;

    let (pm, seen, _queue) = build_process_manager(harness.pool.clone());
    pm.enqueue_effects_for(std::slice::from_ref(&env))
        .await
        .expect("enqueue effects");

    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE queue = $1")
        .bind(AGGREGATE)
        .fetch_one(&harness.pool)
        .await
        .expect("count jobs");
    assert_eq!(pending, 1, "policy must emit exactly one job");

    let dispatched = pm.claim_and_dispatch_one().await.expect("dispatch");
    assert!(dispatched);
    assert_eq!(
        seen.lock().expect("lock").as_slice(),
        &[CounterEffect { by: 5 }]
    );

    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE queue = $1")
        .bind(AGGREGATE)
        .fetch_one(&harness.pool)
        .await
        .expect("count jobs after ack");
    assert_eq!(remaining, 0, "ack must delete job after success");

    let none = pm.claim_and_dispatch_one().await.expect("dispatch empty");
    assert!(!none, "no more jobs left");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_envelope_log_positions_do_not_double_enqueue() {
    let harness = common::start().await;
    let stream = Uuid::now_v7();
    let env = append_event(&harness.pool, stream, 0, 3).await;

    let (pm, _seen, _queue) = build_process_manager(harness.pool.clone());
    pm.enqueue_effects_for(std::slice::from_ref(&env))
        .await
        .expect("enqueue 1");
    pm.enqueue_effects_for(std::slice::from_ref(&env))
        .await
        .expect("enqueue 2");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE queue = $1")
        .bind(AGGREGATE)
        .fetch_one(&harness.pool)
        .await
        .expect("count");
    assert_eq!(
        count, 1,
        "idempotency_key must dedupe across repeated process-manager runs"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enqueue_effects_skips_when_aggregate_state_is_missing() {
    let harness = common::start().await;
    let (pm, _seen, _queue) = build_process_manager(harness.pool.clone());

    let fake_stream = Uuid::now_v7();
    let envelope = EventEnvelope {
        event: CounterEvent::Incremented { by: 1 },
        metadata: EventMetadata {
            stream_id: fake_stream,
            aggregate_type: AGGREGATE.to_string(),
            sequence: 1,
            log_position: 1,
            event_type: "Incremented".to_string(),
            occurred_at: "2026-05-18T00:00:00Z".to_string(),
        },
    };

    pm.enqueue_effects_for(&[envelope])
        .await
        .expect("enqueue with no aggregate");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE queue = $1")
        .bind(AGGREGATE)
        .fetch_one(&harness.pool)
        .await
        .expect("count");
    assert_eq!(
        count, 0,
        "policy fan-out must skip envelopes whose aggregate is not yet persisted"
    );
}
