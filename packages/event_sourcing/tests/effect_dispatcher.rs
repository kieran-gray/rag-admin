#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Notify;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::effect_dispatcher::EffectDispatcher;
use event_sourcing::event_store::EventStore;
use event_sourcing::job_queue::JobQueue;
use event_sourcing::process_manager::{EffectExecutor, ProcessManager};

use common::{
    Counter, CounterEvent, InMemEventStore, InMemJobQueue, InMemSnapshotStore, JobState,
    RecordingExecutor, TestEffect,
};

#[tokio::test]
async fn dispatcher_run_drains_enqueued_jobs() {
    let store = Arc::new(InMemEventStore::empty());
    let stream = Uuid::now_v7();
    let envs = store
        .append(stream, 0, &vec![CounterEvent::Inc, CounterEvent::Inc])
        .await
        .expect("append");

    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let executor = Arc::new(RecordingExecutor::new());

    let event_store: Arc<dyn EventStore<CounterEvent>> = store.clone();
    let repo = Arc::new(AggregateRepository::<Counter>::new(
        event_store,
        Arc::new(InMemSnapshotStore::empty()),
    ));
    let pm_queue: Arc<dyn JobQueue<TestEffect>> = queue.clone();
    let pm_executor: Arc<dyn EffectExecutor<TestEffect>> = executor.clone();
    let pm = Arc::new(ProcessManager::<Counter, TestEffect>::new(
        repo,
        pm_queue,
        pm_executor,
    ));

    pm.enqueue_effects_for(&envs).await.expect("enqueue");
    assert_eq!(queue.count_by_state(JobState::Pending), 2);

    let wakeup = Arc::new(Notify::new());
    let dispatcher = Arc::new(EffectDispatcher::new(Arc::clone(&pm), Arc::clone(&wakeup)));

    let handle = tokio::spawn(Arc::clone(&dispatcher).run());
    wakeup.notify_one();

    let deadline = Instant::now() + Duration::from_secs(2);
    while executor.calls().len() < 2 && Instant::now() < deadline {
        sleep(Duration::from_millis(20)).await;
    }

    handle.abort();
    let _ = timeout(Duration::from_millis(200), handle).await;

    assert_eq!(executor.calls().len(), 2);
    assert_eq!(queue.count_by_state(JobState::Acked), 2);
}

#[tokio::test]
async fn dispatcher_run_wakes_on_notify_after_idle() {
    let store = Arc::new(InMemEventStore::empty());
    let queue = Arc::new(InMemJobQueue::<TestEffect>::new());
    let executor = Arc::new(RecordingExecutor::new());

    let event_store: Arc<dyn EventStore<CounterEvent>> = store.clone();
    let repo = Arc::new(AggregateRepository::<Counter>::new(
        event_store,
        Arc::new(InMemSnapshotStore::empty()),
    ));
    let pm_queue: Arc<dyn JobQueue<TestEffect>> = queue.clone();
    let pm_executor: Arc<dyn EffectExecutor<TestEffect>> = executor.clone();
    let pm = Arc::new(ProcessManager::<Counter, TestEffect>::new(
        repo,
        pm_queue,
        pm_executor,
    ));

    let wakeup = Arc::new(Notify::new());
    let dispatcher = Arc::new(EffectDispatcher::new(Arc::clone(&pm), Arc::clone(&wakeup)));
    let handle = tokio::spawn(Arc::clone(&dispatcher).run());

    // Let it idle, then enqueue work and notify.
    sleep(Duration::from_millis(50)).await;
    let stream = Uuid::now_v7();
    let envs = store
        .append(stream, 0, &vec![CounterEvent::Inc])
        .await
        .expect("append");
    pm.enqueue_effects_for(&envs).await.expect("enqueue");
    wakeup.notify_one();

    let deadline = Instant::now() + Duration::from_secs(2);
    while executor.calls().len() < 1 && Instant::now() < deadline {
        sleep(Duration::from_millis(20)).await;
    }

    handle.abort();
    let _ = timeout(Duration::from_millis(200), handle).await;

    assert_eq!(executor.calls().len(), 1);
}
