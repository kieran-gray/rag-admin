#![cfg(feature = "ssr")]

mod common;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use event_sourcing::error::EsError;
use event_sourcing::event_store::EventStore;
use rag_admin::server::infrastructure::event_sourcing::PostgresEventStore;

const AGGREGATE: &str = "test_aggregate";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum TestEvent {
    Created { name: String },
    Updated { value: i64 },
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn append_assigns_monotonic_positions_and_global_log_ordering() {
    let harness = common::start().await;
    let store = PostgresEventStore::<TestEvent>::new(harness.pool.clone(), AGGREGATE);

    let stream_a = Uuid::now_v7();
    let stream_b = Uuid::now_v7();

    let appended_a = store
        .append(
            stream_a,
            0,
            &[
                TestEvent::Created { name: "a".into() },
                TestEvent::Updated { value: 1 },
            ],
        )
        .await
        .expect("append stream A");
    assert_eq!(appended_a.len(), 2);
    assert_eq!(appended_a[0].metadata.sequence, 1);
    assert_eq!(appended_a[1].metadata.sequence, 2);
    assert!(appended_a[1].metadata.log_position > appended_a[0].metadata.log_position);

    let appended_b = store
        .append(stream_b, 0, &[TestEvent::Created { name: "b".into() }])
        .await
        .expect("append stream B");
    assert_eq!(appended_b[0].metadata.sequence, 1);
    assert!(appended_b[0].metadata.log_position > appended_a[1].metadata.log_position);

    let loaded = store.load(stream_a).await.expect("load A");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].event, TestEvent::Created { name: "a".into() });
    assert_eq!(loaded[1].event, TestEvent::Updated { value: 1 });

    let after_first = store.load_after(stream_a, 1).await.expect("load_after A");
    assert_eq!(after_first.len(), 1);
    assert_eq!(after_first[0].metadata.sequence, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn append_with_wrong_expected_version_returns_concurrency_conflict() {
    let harness = common::start().await;
    let store = PostgresEventStore::<TestEvent>::new(harness.pool.clone(), AGGREGATE);
    let stream = Uuid::now_v7();

    store
        .append(stream, 0, &[TestEvent::Created { name: "x".into() }])
        .await
        .expect("first append");

    let err = store
        .append(stream, 0, &[TestEvent::Updated { value: 99 }])
        .await
        .expect_err("stale expected_version should fail");

    match err {
        EsError::ConcurrencyConflict {
            expected, actual, ..
        } => {
            assert_eq!(expected, 0);
            assert_eq!(actual, 1);
        }
        other => panic!("expected ConcurrencyConflict, got {other:?}"),
    }

    let loaded = store.load(stream).await.expect("load");
    assert_eq!(loaded.len(), 1, "failed append must not commit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_appends_to_same_stream_only_one_wins() {
    let harness = common::start().await;
    let store_a = PostgresEventStore::<TestEvent>::new(harness.pool.clone(), AGGREGATE);
    let store_b = PostgresEventStore::<TestEvent>::new(harness.pool.clone(), AGGREGATE);
    let stream = Uuid::now_v7();

    let a = tokio::spawn(async move {
        store_a
            .append(stream, 0, &[TestEvent::Created { name: "a".into() }])
            .await
    });
    let b = tokio::spawn(async move {
        store_b
            .append(stream, 0, &[TestEvent::Created { name: "b".into() }])
            .await
    });

    let ra = a.await.expect("join a");
    let rb = b.await.expect("join b");
    let mut successes = 0;
    let mut conflicts = 0;
    for r in [ra, rb] {
        match r {
            Ok(events) => {
                successes += 1;
                assert_eq!(events.len(), 1);
            }
            Err(EsError::ConcurrencyConflict { .. }) => conflicts += 1,
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!(successes, 1, "exactly one append must win");
    assert_eq!(conflicts, 1, "the other must see ConcurrencyConflict");

    let final_store = PostgresEventStore::<TestEvent>::new(harness.pool.clone(), AGGREGATE);
    let loaded = final_store.load(stream).await.expect("load");
    assert_eq!(loaded.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn load_global_after_paginates_in_log_order_and_filters_by_aggregate_type() {
    let harness = common::start().await;
    let store = PostgresEventStore::<TestEvent>::new(harness.pool.clone(), AGGREGATE);
    let other = PostgresEventStore::<TestEvent>::new(harness.pool.clone(), "other_aggregate");

    let stream_a = Uuid::now_v7();
    let stream_b = Uuid::now_v7();
    store
        .append(
            stream_a,
            0,
            &[
                TestEvent::Created { name: "a1".into() },
                TestEvent::Created { name: "a2".into() },
            ],
        )
        .await
        .expect("append A");
    other
        .append(stream_b, 0, &[TestEvent::Created { name: "x".into() }])
        .await
        .expect("append other");
    store
        .append(stream_a, 2, &[TestEvent::Updated { value: 3 }])
        .await
        .expect("append A again");

    let first_page = store.load_global_after(0, 2).await.expect("global page 1");
    assert_eq!(first_page.len(), 2);
    assert!(first_page[0].metadata.log_position < first_page[1].metadata.log_position);
    for env in &first_page {
        assert_eq!(env.metadata.aggregate_type, AGGREGATE);
    }

    let cursor = first_page
        .last()
        .map(|e| e.metadata.log_position)
        .expect("page has events");
    let next_page = store
        .load_global_after(cursor, 10)
        .await
        .expect("global page 2");
    assert_eq!(next_page.len(), 1);
    assert_eq!(next_page[0].event, TestEvent::Updated { value: 3 });
}
