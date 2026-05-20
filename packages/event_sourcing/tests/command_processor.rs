#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;

use uuid::Uuid;

use event_sourcing::aggregate_repository::AggregateRepository;
use event_sourcing::command_processor::CommandProcessor;
use event_sourcing::error::CommandError;
use event_sourcing::event_store::EventStore;

use common::{
    ConflictingEventStore, Counter, CounterCommand, CounterError, CounterEvent, InMemEventStore,
    InMemSnapshotStore,
};

fn build_processor(
    store: impl EventStore<CounterEvent> + 'static,
    snaps: Arc<InMemSnapshotStore>,
) -> CommandProcessor<Counter> {
    let event_store: Arc<dyn EventStore<CounterEvent>> = Arc::new(store);
    let repo = Arc::new(AggregateRepository::new(event_store, snaps));
    CommandProcessor::new(repo)
}

#[tokio::test]
async fn handle_appends_events_for_new_stream() {
    let cp = build_processor(
        InMemEventStore::empty(),
        Arc::new(InMemSnapshotStore::empty()),
    );
    let stream = Uuid::now_v7();

    let envs = cp
        .handle(
            stream,
            CounterCommand {
                events: vec![CounterEvent::Inc, CounterEvent::Inc],
                fail_with: None,
            },
        )
        .await
        .expect("handle");

    assert_eq!(envs.len(), 2);
    assert_eq!(envs[0].metadata.sequence, 1);
    assert_eq!(envs[1].metadata.sequence, 2);
    assert_eq!(envs[0].metadata.stream_id, stream);
}

#[tokio::test]
async fn handle_returns_empty_when_command_produces_no_events() {
    let cp = build_processor(
        InMemEventStore::empty(),
        Arc::new(InMemSnapshotStore::empty()),
    );

    let envs = cp
        .handle(
            Uuid::now_v7(),
            CounterCommand {
                events: vec![],
                fail_with: None,
            },
        )
        .await
        .expect("handle");

    assert!(envs.is_empty());
}

#[tokio::test]
async fn aggregate_error_is_wrapped_in_command_error_aggregate_variant() {
    let cp = build_processor(
        InMemEventStore::empty(),
        Arc::new(InMemSnapshotStore::empty()),
    );

    let err = cp
        .handle(
            Uuid::now_v7(),
            CounterCommand {
                events: vec![],
                fail_with: Some(CounterError("rejected".into())),
            },
        )
        .await
        .expect_err("should fail");

    assert!(matches!(err, CommandError::Aggregate(_)));
}

#[tokio::test]
async fn snapshot_saved_after_crossing_threshold() {
    // SNAPSHOT_AFTER_EVENTS = 16 in command_processor.rs
    let snaps = Arc::new(InMemSnapshotStore::empty());
    let cp = build_processor(InMemEventStore::empty(), Arc::clone(&snaps));
    let stream = Uuid::now_v7();

    cp.handle(
        stream,
        CounterCommand {
            events: vec![CounterEvent::Inc; 15],
            fail_with: None,
        },
    )
    .await
    .expect("handle");

    assert!(snaps.get().is_none());
    cp.handle(
        stream,
        CounterCommand {
            events: vec![CounterEvent::Inc],
            fail_with: None,
        },
    )
    .await
    .expect("handle");

    let snap = snaps.get().expect("snapshot saved after threshold");
    assert_eq!(snap.stream_id, stream);
    assert_eq!(snap.version, 16);
    assert_eq!(snap.aggregate.count, 16);
}

#[tokio::test]
async fn concurrency_conflict_propagates_as_event_store_error() {
    let cp = build_processor(ConflictingEventStore, Arc::new(InMemSnapshotStore::empty()));

    let err = cp
        .handle(
            Uuid::now_v7(),
            CounterCommand {
                events: vec![CounterEvent::Inc],
                fail_with: None,
            },
        )
        .await
        .expect_err("should fail");

    assert!(matches!(err, CommandError::EventStore(_)));
}
