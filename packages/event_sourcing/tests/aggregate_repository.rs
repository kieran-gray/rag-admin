#![allow(clippy::expect_used)]

mod common;

use std::sync::Arc;

use uuid::Uuid;

use event_sourcing::aggregate_repository::{AggregateRepository, AggregateSnapshot};

use common::{make_envelope, Counter, InMemEventStore, InMemSnapshotStore};

fn repo(store: InMemEventStore, snaps: InMemSnapshotStore) -> AggregateRepository<Counter> {
    AggregateRepository::new(Arc::new(store), Arc::new(snaps))
}

#[tokio::test]
async fn load_returns_none_for_stream_with_no_events_and_no_snapshot() {
    let r = repo(InMemEventStore::empty(), InMemSnapshotStore::empty());
    assert!(r.load(Uuid::now_v7()).await.expect("load").is_none());
}

#[tokio::test]
async fn load_builds_aggregate_from_events_when_no_snapshot() {
    let stream = Uuid::now_v7();
    let events = vec![make_envelope(stream, 1), make_envelope(stream, 2)];
    let r = repo(InMemEventStore::with(events), InMemSnapshotStore::empty());

    let loaded = r.load(stream).await.expect("load").expect("some");

    assert_eq!(loaded.aggregate.count, 2);
    assert_eq!(loaded.version, 2);
    assert_eq!(loaded.new_events_since_snapshot, 2);
}

#[tokio::test]
async fn load_applies_events_on_top_of_snapshot() {
    let stream = Uuid::now_v7();
    let snap = AggregateSnapshot {
        stream_id: stream,
        version: 3,
        aggregate: Counter { count: 3 },
    };
    let events = vec![make_envelope(stream, 4), make_envelope(stream, 5)];
    let r = repo(
        InMemEventStore::with(events),
        InMemSnapshotStore::with(snap),
    );

    let loaded = r.load(stream).await.expect("load").expect("some");

    assert_eq!(loaded.aggregate.count, 5);
    assert_eq!(loaded.version, 5);
    assert_eq!(loaded.new_events_since_snapshot, 2);
}

#[tokio::test]
async fn load_with_snapshot_and_no_newer_events_returns_snapshot_state() {
    let stream = Uuid::now_v7();
    let snap = AggregateSnapshot {
        stream_id: stream,
        version: 5,
        aggregate: Counter { count: 5 },
    };
    let r = repo(InMemEventStore::empty(), InMemSnapshotStore::with(snap));

    let loaded = r.load(stream).await.expect("load").expect("some");

    assert_eq!(loaded.aggregate.count, 5);
    assert_eq!(loaded.version, 5);
    assert_eq!(loaded.new_events_since_snapshot, 0);
}

#[tokio::test]
async fn save_snapshot_is_visible_to_subsequent_load() {
    let stream = Uuid::now_v7();
    let snaps = Arc::new(InMemSnapshotStore::empty());
    let r = AggregateRepository::<Counter>::new(Arc::new(InMemEventStore::empty()), snaps);

    r.save_snapshot(stream, 7, &Counter { count: 7 })
        .await
        .expect("save");

    let loaded = r.load(stream).await.expect("load").expect("some");
    assert_eq!(loaded.aggregate.count, 7);
    assert_eq!(loaded.version, 7);
    assert_eq!(loaded.new_events_since_snapshot, 0);
}
