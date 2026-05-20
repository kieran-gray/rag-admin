#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio::time::timeout;
use uuid::Uuid;

use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::checkpoint::{CheckpointRepository, CheckpointStatus, ProjectionCheckpoint};
use event_sourcing::event_bus::EventBus;
use event_sourcing::event_store::EventStore;
use event_sourcing::process_manager::ProcessManager;
use event_sourcing::projection_driver::ProjectionDriver;
use event_sourcing::projector::Projector;

use common::{
    Counter, CounterEvent, InMemCheckpointRepository, InMemEventStore, InMemJobQueue,
    InMemSnapshotStore, JobState, RecordingExecutor, RecordingProjector, TestEffect,
};

async fn append(store: &InMemEventStore, stream: Uuid, n: usize) {
    store
        .append(stream, 0, &vec![CounterEvent::Inc; n])
        .await
        .expect("append");
}

fn make_driver(
    store: Arc<InMemEventStore>,
    projectors: Vec<Arc<dyn Projector<CounterEvent>>>,
    checkpoints: Arc<InMemCheckpointRepository>,
    bus: Arc<EventBus>,
    pm: Option<Arc<ProcessManager<Counter, TestEffect>>>,
    projection_wakeup: Arc<Notify>,
    effect_wakeup: Arc<Notify>,
) -> ProjectionDriver<Counter, TestEffect> {
    ProjectionDriver::new(
        store,
        projectors,
        checkpoints,
        bus,
        pm,
        projection_wakeup,
        effect_wakeup,
    )
}

#[tokio::test]
async fn tick_returns_false_when_no_events() {
    let store = Arc::new(InMemEventStore::empty());
    let proj = Arc::new(RecordingProjector::new("p1"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());

    let driver = make_driver(
        store,
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        checkpoints,
        bus,
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    assert!(!driver.tick().await.expect("tick"));
    assert!(proj.recorded().is_empty());
}

#[tokio::test]
async fn tick_advances_checkpoint_on_success() {
    let store = Arc::new(InMemEventStore::empty());
    let stream = Uuid::now_v7();
    append(&store, stream, 2).await;

    let proj = Arc::new(RecordingProjector::new("p1"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());

    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        bus,
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    assert!(driver.tick().await.expect("tick"));
    let recorded = proj.recorded();
    assert_eq!(recorded.len(), 2);

    let cp = checkpoints
        .load("p1")
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(cp.status, CheckpointStatus::Healthy);
    assert_eq!(cp.last_processed_log_position, 2);
    assert_eq!(cp.error_count, 0);
    assert!(cp.error_message.is_none());
}

#[tokio::test]
async fn tick_advances_from_existing_checkpoint() {
    let store = Arc::new(InMemEventStore::empty());
    let stream = Uuid::now_v7();
    append(&store, stream, 3).await;

    let proj = Arc::new(RecordingProjector::new("p1"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    checkpoints.put(ProjectionCheckpoint {
        projector_name: "p1".into(),
        last_processed_log_position: 1,
        status: CheckpointStatus::Healthy,
        error_message: None,
        error_count: 0,
        updated_at: OffsetDateTime::now_utc(),
    });
    let bus = Arc::new(EventBus::new());

    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        bus,
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    assert!(driver.tick().await.expect("tick"));
    let recorded = proj.recorded();
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0].metadata.log_position, 2);
    assert_eq!(recorded[1].metadata.log_position, 3);

    let cp = checkpoints
        .load("p1")
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(cp.last_processed_log_position, 3);
}

#[tokio::test]
async fn tick_increments_error_count_on_projector_failure() {
    let store = Arc::new(InMemEventStore::empty());
    append(&store, Uuid::now_v7(), 1).await;

    let proj = Arc::new(RecordingProjector::new("p1"));
    proj.fail_next(1);
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());

    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        bus,
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    assert!(driver.tick().await.expect("tick"));

    let cp = checkpoints
        .load("p1")
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(cp.status, CheckpointStatus::Error);
    assert_eq!(cp.error_count, 1);
    assert_eq!(cp.last_processed_log_position, 0);
    assert!(cp.error_message.is_some());
}

#[tokio::test]
async fn tick_skips_faulted_projector() {
    let store = Arc::new(InMemEventStore::empty());
    append(&store, Uuid::now_v7(), 1).await;

    let proj = Arc::new(RecordingProjector::new("faulted"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    checkpoints.put(ProjectionCheckpoint {
        projector_name: "faulted".into(),
        last_processed_log_position: 0,
        status: CheckpointStatus::Error,
        error_message: Some("dead".into()),
        error_count: 5,
        updated_at: OffsetDateTime::now_utc(),
    });
    let bus = Arc::new(EventBus::new());

    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        bus,
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    assert!(!driver.tick().await.expect("tick"));
    assert!(proj.recorded().is_empty());

    let cp = checkpoints
        .load("faulted")
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(cp.last_processed_log_position, 0);
    assert_eq!(cp.error_count, 5);
}

#[tokio::test]
async fn tick_publishes_to_event_bus_on_success() {
    let store = Arc::new(InMemEventStore::empty());
    let stream = Uuid::now_v7();
    append(&store, stream, 1).await;

    let proj = Arc::new(RecordingProjector::new("p1"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());
    let mut sub = bus.subscribe();

    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        Arc::clone(&bus),
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    driver.tick().await.expect("tick");

    let published = timeout(Duration::from_secs(1), sub.recv())
        .await
        .expect("timeout")
        .expect("recv");
    assert_eq!(published.stream_id, stream);
    assert_eq!(published.aggregate_type, "counter_test");
    assert_eq!(published.event_type, "Inc");
}

#[tokio::test]
async fn tick_does_not_broadcast_on_projector_failure() {
    let store = Arc::new(InMemEventStore::empty());
    append(&store, Uuid::now_v7(), 1).await;

    let proj = Arc::new(RecordingProjector::new("p1"));
    proj.fail_next(1);
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());
    let mut sub = bus.subscribe();

    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        Arc::clone(&bus),
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    driver.tick().await.expect("tick");
    assert!(timeout(Duration::from_millis(50), sub.recv())
        .await
        .is_err());
}

#[tokio::test]
async fn tick_enqueues_effects_and_notifies_when_process_manager_present() {
    let store = Arc::new(InMemEventStore::empty());
    let stream = Uuid::now_v7();
    append(&store, stream, 2).await;

    let proj = Arc::new(RecordingProjector::new("p1"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());

    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let event_store: Arc<dyn EventStore<CounterEvent>> = store.clone();
    let repo = Arc::new(AggregateRepository::<Counter>::new(
        event_store,
        Arc::new(InMemSnapshotStore::empty()),
    ));
    let pm_queue: Arc<dyn event_sourcing::job_queue::JobQueue<TestEffect>> = queue.clone();
    let pm_executor: Arc<dyn event_sourcing::process_manager::EffectExecutor<TestEffect>> =
        Arc::new(RecordingExecutor::new());
    let pm = Arc::new(ProcessManager::<Counter, TestEffect>::new(
        repo,
        pm_queue,
        pm_executor,
    ));

    let effect_wakeup = Arc::new(Notify::new());

    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        bus,
        Some(pm),
        Arc::new(Notify::new()),
        Arc::clone(&effect_wakeup),
    );

    driver.tick().await.expect("tick");

    let jobs = queue.snapshot();
    assert_eq!(jobs.len(), 2);
    assert_eq!(queue.count_by_state(JobState::Pending), 2);
    assert!(
        timeout(Duration::from_millis(100), effect_wakeup.notified())
            .await
            .is_ok(),
        "effect_wakeup should have been notified",
    );
}

#[tokio::test]
async fn tick_initializes_checkpoint_when_none_exists() {
    let store = Arc::new(InMemEventStore::empty());
    append(&store, Uuid::now_v7(), 1).await;

    let proj = Arc::new(RecordingProjector::new("fresh"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    assert!(checkpoints.snapshot().is_empty());

    let bus = Arc::new(EventBus::new());
    let driver = make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        bus,
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    driver.tick().await.expect("tick");

    let snap = checkpoints.snapshot();
    assert_eq!(snap.len(), 1);
    let cp = snap.get("fresh").expect("present");
    assert_eq!(cp.last_processed_log_position, 1);
}

#[tokio::test]
async fn run_drains_pending_events_when_notified() {
    let store = Arc::new(InMemEventStore::empty());
    let proj = Arc::new(RecordingProjector::new("p1"));
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());
    let projection_wakeup = Arc::new(Notify::new());

    let driver = Arc::new(make_driver(
        Arc::clone(&store),
        vec![Arc::clone(&proj) as Arc<dyn Projector<CounterEvent>>],
        Arc::clone(&checkpoints),
        bus,
        None,
        Arc::clone(&projection_wakeup),
        Arc::new(Notify::new()),
    ));

    let handle = tokio::spawn(Arc::clone(&driver).run());

    let stream = Uuid::now_v7();
    append(&store, stream, 2).await;
    projection_wakeup.notify_one();

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while proj.recorded().len() < 2 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    handle.abort();
    let _ = timeout(Duration::from_millis(200), handle).await;

    assert_eq!(proj.recorded().len(), 2);
    let cp = checkpoints.load("p1").await.expect("load").expect("cp");
    assert_eq!(cp.last_processed_log_position, 2);
}

#[tokio::test]
async fn tick_runs_multiple_projectors_independently() {
    let store = Arc::new(InMemEventStore::empty());
    append(&store, Uuid::now_v7(), 2).await;

    let good = Arc::new(RecordingProjector::new("good"));
    let bad = Arc::new(RecordingProjector::new("bad"));
    bad.fail_next(1);
    let checkpoints = Arc::new(InMemCheckpointRepository::new());
    let bus = Arc::new(EventBus::new());

    let driver = make_driver(
        Arc::clone(&store),
        vec![
            Arc::clone(&good) as Arc<dyn Projector<CounterEvent>>,
            Arc::clone(&bad) as Arc<dyn Projector<CounterEvent>>,
        ],
        Arc::clone(&checkpoints),
        bus,
        None,
        Arc::new(Notify::new()),
        Arc::new(Notify::new()),
    );

    driver.tick().await.expect("tick");

    assert_eq!(good.recorded().len(), 2);
    let good_cp = checkpoints
        .load("good")
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(good_cp.status, CheckpointStatus::Healthy);
    let bad_cp = checkpoints
        .load("bad")
        .await
        .expect("load")
        .expect("checkpoint");
    assert_eq!(bad_cp.status, CheckpointStatus::Error);
    assert_eq!(bad_cp.error_count, 1);
}
