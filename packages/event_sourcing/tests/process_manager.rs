#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use uuid::Uuid;

use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::envelope::EventEnvelope;
use event_sourcing::event_store::EventStore;
use event_sourcing::process_manager::ProcessManager;

use common::{
    make_envelope, Counter, CounterEvent, InMemEventStore, InMemJobQueue, InMemSnapshotStore,
    JobState, RecordingExecutor, TestEffect,
};

async fn append_events(
    store: &InMemEventStore,
    stream: Uuid,
    n: usize,
) -> Vec<EventEnvelope<CounterEvent>> {
    store
        .append(stream, 0, &vec![CounterEvent::Inc; n])
        .await
        .expect("append")
}

fn build_pm(
    store: Arc<InMemEventStore>,
    queue: Arc<InMemJobQueue<TestEffect>>,
    executor: Arc<RecordingExecutor>,
) -> ProcessManager<Counter, TestEffect> {
    let repo = Arc::new(AggregateRepository::<Counter>::new(
        store,
        Arc::new(InMemSnapshotStore::empty()),
    ));
    ProcessManager::new(repo, queue, executor)
}

#[tokio::test]
async fn enqueue_effects_for_empty_is_noop() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::new(RecordingExecutor::new()),
    );

    pm.enqueue_effects_for(&[]).await.expect("ok");
    assert_eq!(queue.snapshot().len(), 0);
}

#[tokio::test]
async fn enqueue_effects_for_skips_unknown_aggregate() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::new(RecordingExecutor::new()),
    );

    let env = make_envelope(Uuid::now_v7(), 1);
    pm.enqueue_effects_for(&[env]).await.expect("ok");
    assert_eq!(queue.snapshot().len(), 0);
}

#[tokio::test]
async fn enqueue_effects_for_creates_jobs_from_policies() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::new(RecordingExecutor::new()),
    );

    let stream = Uuid::now_v7();
    let envs = append_events(&store, stream, 3).await;
    pm.enqueue_effects_for(&envs).await.expect("ok");

    let jobs = queue.snapshot();
    assert_eq!(jobs.len(), 3);
    assert!(jobs.iter().all(|j| j.job_type == "test_effect"));
    assert!(jobs.iter().all(|j| j.partition_key == Some(stream)));
    let mut positions: Vec<i64> = jobs.iter().map(|j| j.payload.log_position).collect();
    positions.sort();
    assert_eq!(positions, vec![1, 2, 3]);
}

#[tokio::test]
async fn enqueue_effects_for_is_idempotent_across_calls() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::new(RecordingExecutor::new()),
    );

    let stream = Uuid::now_v7();
    let envs = append_events(&store, stream, 2).await;
    pm.enqueue_effects_for(&envs).await.expect("ok");
    pm.enqueue_effects_for(&envs).await.expect("ok");
    assert_eq!(queue.snapshot().len(), 2);
}

#[tokio::test]
async fn enqueue_effects_for_groups_by_stream() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::new(RecordingExecutor::new()),
    );

    let stream_a = Uuid::now_v7();
    let stream_b = Uuid::now_v7();
    let mut envs = append_events(&store, stream_a, 1).await;
    envs.extend(append_events(&store, stream_b, 2).await);
    pm.enqueue_effects_for(&envs).await.expect("ok");

    let jobs = queue.snapshot();
    let a_count = jobs
        .iter()
        .filter(|j| j.partition_key == Some(stream_a))
        .count();
    let b_count = jobs
        .iter()
        .filter(|j| j.partition_key == Some(stream_b))
        .count();
    assert_eq!(a_count, 1);
    assert_eq!(b_count, 2);
}

#[tokio::test]
async fn claim_and_dispatch_returns_false_when_queue_empty() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::new(RecordingExecutor::new()),
    );

    assert!(!pm.claim_and_dispatch_one().await.expect("ok"));
}

#[tokio::test]
async fn claim_and_dispatch_acks_job_on_success() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let executor = Arc::new(RecordingExecutor::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::clone(&executor),
    );

    let stream = Uuid::now_v7();
    let envs = append_events(&store, stream, 1).await;
    pm.enqueue_effects_for(&envs).await.expect("ok");

    let processed = pm.claim_and_dispatch_one().await.expect("ok");
    assert!(processed);
    assert_eq!(queue.count_by_state(JobState::Acked), 1);
    assert_eq!(queue.count_by_state(JobState::Pending), 0);
    let calls = executor.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].log_position, 1);
}

#[tokio::test]
async fn claim_and_dispatch_nacks_on_failure_with_backoff() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let executor = Arc::new(RecordingExecutor::new());
    executor.fail_next(1);
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::clone(&executor),
    );

    let stream = Uuid::now_v7();
    let envs = append_events(&store, stream, 1).await;
    pm.enqueue_effects_for(&envs).await.expect("ok");

    let before = SystemTime::now();
    assert!(pm.claim_and_dispatch_one().await.expect("ok"));

    let jobs = queue.snapshot();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].state, JobState::Pending);
    assert_eq!(jobs[0].attempts, 1);
    assert!(jobs[0].available_at > before + Duration::from_secs(1));
    assert_eq!(
        jobs[0].last_error.as_deref(),
        Some("forced executor failure")
    );
}

#[tokio::test]
async fn claim_and_dispatch_dead_letters_when_attempts_exhausted() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::with_max_attempts(1));
    let executor = Arc::new(RecordingExecutor::new());
    executor.fail_next(1);
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::clone(&executor),
    );

    let stream = Uuid::now_v7();
    let envs = append_events(&store, stream, 1).await;
    pm.enqueue_effects_for(&envs).await.expect("ok");

    assert!(pm.claim_and_dispatch_one().await.expect("ok"));

    let jobs = queue.snapshot();
    assert_eq!(jobs[0].state, JobState::Dead);
    assert_eq!(
        jobs[0].last_error.as_deref(),
        Some("forced executor failure")
    );
    assert_eq!(queue.count_by_state(JobState::Acked), 0);
}

#[tokio::test]
async fn claim_and_dispatch_processes_one_job_at_a_time() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let executor = Arc::new(RecordingExecutor::new());
    let pm = build_pm(
        Arc::clone(&store),
        Arc::clone(&queue),
        Arc::clone(&executor),
    );

    let stream = Uuid::now_v7();
    let envs = append_events(&store, stream, 3).await;
    pm.enqueue_effects_for(&envs).await.expect("ok");

    assert!(pm.claim_and_dispatch_one().await.expect("ok"));
    assert_eq!(queue.count_by_state(JobState::Acked), 1);
    assert_eq!(queue.count_by_state(JobState::Pending), 2);

    assert!(pm.claim_and_dispatch_one().await.expect("ok"));
    assert!(pm.claim_and_dispatch_one().await.expect("ok"));
    assert_eq!(queue.count_by_state(JobState::Acked), 3);

    assert!(!pm.claim_and_dispatch_one().await.expect("ok"));
}
