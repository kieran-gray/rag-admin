#![cfg(feature = "ssr")]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use uuid::Uuid;

use event_sourcing::aggregate::Aggregate;
use event_sourcing::checkpoint::CheckpointRepository;
use event_sourcing::envelope::EventEnvelope;
use event_sourcing::error::ProjectionError;
use event_sourcing::event_bus::EventBus;
use event_sourcing::event_store::EventStore;
use event_sourcing::policy::HasPolicies;
use event_sourcing::projection_driver::ProjectionDriver;
use event_sourcing::projector::Projector;
use rag_admin::server::infrastructure::event_sourcing::{
    PostgresCheckpointRepository, PostgresEventStore,
};

const AGGREGATE: &str = "projection_driver_test";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TestAggregate;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum TestEvent {
    Tick { n: i64 },
}

#[derive(Debug, thiserror::Error)]
#[error("never")]
struct TestError;

impl HasPolicies<TestAggregate, ()> for TestEvent {}

impl Aggregate for TestAggregate {
    type Event = TestEvent;
    type Command = ();
    type Error = TestError;

    fn aggregate_type() -> &'static str {
        AGGREGATE
    }

    fn apply(&mut self, _event: &Self::Event) {}

    fn handle_command(
        _state: Option<&Self>,
        _command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        Ok(Vec::new())
    }

    fn from_events(events: &[Self::Event]) -> Option<Self> {
        if events.is_empty() {
            None
        } else {
            Some(Self)
        }
    }
}

struct RecordingProjector {
    name: &'static str,
    seen: Arc<Mutex<Vec<i64>>>,
}

#[async_trait]
impl Projector<TestEvent> for RecordingProjector {
    fn name(&self) -> &str {
        self.name
    }

    async fn project(&self, events: &[EventEnvelope<TestEvent>]) -> Result<(), ProjectionError> {
        let mut guard = self.seen.lock().expect("lock");
        for env in events {
            let TestEvent::Tick { n } = env.event;
            guard.push(n);
        }
        Ok(())
    }
}

struct FlakyProjector {
    name: &'static str,
    fail_until: AtomicUsize,
    attempts: AtomicUsize,
}

#[async_trait]
impl Projector<TestEvent> for FlakyProjector {
    fn name(&self) -> &str {
        self.name
    }

    async fn project(&self, _events: &[EventEnvelope<TestEvent>]) -> Result<(), ProjectionError> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt < self.fail_until.load(Ordering::SeqCst) {
            Err(ProjectionError::Storage(format!(
                "synthetic failure {attempt}"
            )))
        } else {
            Ok(())
        }
    }
}

fn build_driver(
    pool: sqlx::PgPool,
    projectors: Vec<Arc<dyn Projector<TestEvent>>>,
) -> Arc<ProjectionDriver<TestAggregate, ()>> {
    let event_store: Arc<dyn EventStore<TestEvent>> = Arc::new(
        PostgresEventStore::<TestEvent>::new(pool.clone(), AGGREGATE),
    );
    let checkpoints: Arc<dyn CheckpointRepository> =
        Arc::new(PostgresCheckpointRepository::new(pool));
    Arc::new(ProjectionDriver::new(
        event_store,
        projectors,
        checkpoints,
        Arc::new(EventBus::new()),
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    ))
}

async fn append_ticks(pool: &sqlx::PgPool, stream: Uuid, expected: usize, n: usize) {
    let store = PostgresEventStore::<TestEvent>::new(pool.clone(), AGGREGATE);
    let events: Vec<TestEvent> = (0..n)
        .map(|i| TestEvent::Tick {
            n: (expected + i + 1) as i64,
        })
        .collect();
    store
        .append(stream, expected, &events)
        .await
        .expect("append events");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tick_advances_checkpoint_and_invokes_projector() {
    let harness = common::start().await;
    let stream = Uuid::now_v7();
    append_ticks(&harness.pool, stream, 0, 3).await;

    let seen = Arc::new(Mutex::new(Vec::<i64>::new()));
    let projector: Arc<dyn Projector<TestEvent>> = Arc::new(RecordingProjector {
        name: "recorder",
        seen: Arc::clone(&seen),
    });
    let driver = build_driver(harness.pool.clone(), vec![projector]);

    let advanced = driver.tick().await.expect("tick");
    assert!(advanced, "first tick must report work");

    let no_more = driver.tick().await.expect("tick again");
    assert!(!no_more, "second tick must report no further work");

    let recorded = seen.lock().expect("lock").clone();
    assert_eq!(recorded, vec![1, 2, 3]);

    let checkpoints = PostgresCheckpointRepository::new(harness.pool.clone());
    let cp = checkpoints
        .load("recorder")
        .await
        .expect("load checkpoint")
        .expect("checkpoint exists");
    assert!(cp.last_processed_log_position > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn faulted_projector_is_retried_then_skipped() {
    let harness = common::start().await;
    let stream = Uuid::now_v7();
    append_ticks(&harness.pool, stream, 0, 1).await;

    let projector = Arc::new(FlakyProjector {
        name: "flaky",
        fail_until: AtomicUsize::new(10),
        attempts: AtomicUsize::new(0),
    });
    let driver = build_driver(
        harness.pool.clone(),
        vec![Arc::clone(&projector) as Arc<dyn Projector<TestEvent>>],
    );

    for _ in 0..6 {
        driver.tick().await.expect("tick");
    }

    let attempts_after_fault = projector.attempts.load(Ordering::SeqCst);
    assert!(
        (5..=6).contains(&attempts_after_fault),
        "projector should be retried up to MAX_PROJECTOR_ERROR_COUNT (=5) attempts, got {attempts_after_fault}",
    );

    let checkpoints = PostgresCheckpointRepository::new(harness.pool.clone());
    let cp = checkpoints
        .load("flaky")
        .await
        .expect("load checkpoint")
        .expect("checkpoint exists");
    assert_eq!(cp.error_count, 5);
    assert_eq!(cp.last_processed_log_position, 0);

    let further = projector.attempts.load(Ordering::SeqCst);
    driver.tick().await.expect("tick after fault");
    assert_eq!(
        projector.attempts.load(Ordering::SeqCst),
        further,
        "faulted projector must be skipped on subsequent ticks"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn checkpoint_persists_across_driver_restart() {
    let harness = common::start().await;
    let stream = Uuid::now_v7();
    append_ticks(&harness.pool, stream, 0, 2).await;

    let seen_first = Arc::new(Mutex::new(Vec::<i64>::new()));
    let projector_first: Arc<dyn Projector<TestEvent>> = Arc::new(RecordingProjector {
        name: "restartable",
        seen: Arc::clone(&seen_first),
    });
    let driver_one = build_driver(harness.pool.clone(), vec![projector_first]);
    driver_one.tick().await.expect("tick 1");
    drop(driver_one);
    let first_seen = seen_first.lock().expect("lock").clone();
    assert_eq!(first_seen, vec![1, 2]);

    append_ticks(&harness.pool, stream, 2, 2).await;

    let seen_second = Arc::new(Mutex::new(Vec::<i64>::new()));
    let projector_second: Arc<dyn Projector<TestEvent>> = Arc::new(RecordingProjector {
        name: "restartable",
        seen: Arc::clone(&seen_second),
    });
    let driver_two = build_driver(harness.pool.clone(), vec![projector_second]);
    driver_two.tick().await.expect("tick 2");

    let second_seen = seen_second.lock().expect("lock").clone();
    assert_eq!(
        second_seen,
        vec![3, 4],
        "restarted driver must skip events already at-or-before checkpoint"
    );
}
