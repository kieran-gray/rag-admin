#![cfg(feature = "ssr")]

mod common;

use std::time::Duration;

use event_sourcing::{IdempotencyKey, JobQueue, NewJob};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use search_crucible::server::infrastructure::shared::event_sourcing::PostgresJobQueue;

const QUEUE: &str = "test_queue";
const WORKER_A: &str = "worker-a";
const WORKER_B: &str = "worker-b";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TestPayload {
    label: String,
}

fn job(partition: Uuid, discriminator: &str, label: &str) -> NewJob<TestPayload> {
    NewJob {
        partition_key: partition,
        job_type: "test_job",
        idempotency_key: IdempotencyKey::new(partition, 1, discriminator),
        payload: TestPayload {
            label: label.into(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enqueue_is_idempotent_per_queue_and_key() {
    let harness = common::start().await;
    let queue = PostgresJobQueue::<TestPayload>::new(harness.pool.clone());
    let partition = Uuid::now_v7();

    let j = job(partition, "one", "first");
    queue.enqueue(QUEUE, &[j.clone()]).await.expect("enqueue 1");
    queue.enqueue(QUEUE, &[j.clone()]).await.expect("enqueue 2");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE queue = $1")
        .bind(QUEUE)
        .fetch_one(&harness.pool)
        .await
        .expect("count jobs");
    assert_eq!(count, 1, "duplicate idempotency_key must not insert twice");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn claim_ack_nack_with_backoff() {
    let harness = common::start().await;
    let queue = PostgresJobQueue::<TestPayload>::new(harness.pool.clone());
    let partition = Uuid::now_v7();

    queue
        .enqueue(QUEUE, &[job(partition, "ack", "ack-me")])
        .await
        .expect("enqueue ack job");
    queue
        .enqueue(QUEUE, &[job(partition, "nack", "nack-me")])
        .await
        .expect("enqueue nack job");

    let first = queue
        .claim(QUEUE, WORKER_A, Duration::from_secs(30))
        .await
        .expect("claim 1")
        .expect("a job present");
    queue.ack(first.id, WORKER_A).await.expect("ack first job");

    let count_after_ack: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE id = $1")
        .bind(first.id)
        .fetch_one(&harness.pool)
        .await
        .expect("count after ack");
    assert_eq!(count_after_ack, 0, "ack must delete the row");

    let second = queue
        .claim(QUEUE, WORKER_A, Duration::from_secs(30))
        .await
        .expect("claim 2")
        .expect("second job present");
    assert_eq!(second.attempts, 1);

    queue
        .nack(
            second.id,
            WORKER_A,
            "transient failure",
            Duration::from_secs(60),
        )
        .await
        .expect("nack");

    let again = queue
        .claim(QUEUE, WORKER_A, Duration::from_secs(30))
        .await
        .expect("claim after nack");
    assert!(
        again.is_none(),
        "nack with future run_at must hide job from claim"
    );

    let row: (i32, Option<String>) =
        sqlx::query_as("SELECT attempts, last_error FROM jobs WHERE id = $1")
            .bind(second.id)
            .fetch_one(&harness.pool)
            .await
            .expect("read job row after nack");
    assert_eq!(row.0, 1);
    assert_eq!(row.1.as_deref(), Some("transient failure"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn heartbeat_extends_lease_for_owner_and_rejects_other_workers() {
    let harness = common::start().await;
    let queue = PostgresJobQueue::<TestPayload>::new(harness.pool.clone());
    let partition = Uuid::now_v7();
    queue
        .enqueue(QUEUE, &[job(partition, "heartbeat", "long-runner")])
        .await
        .expect("enqueue");

    let claimed = queue
        .claim(QUEUE, WORKER_A, Duration::from_secs(2))
        .await
        .expect("claim")
        .expect("a job");

    let original_lease: time::OffsetDateTime =
        sqlx::query_scalar("SELECT locked_until FROM jobs WHERE id = $1")
            .bind(claimed.id)
            .fetch_one(&harness.pool)
            .await
            .expect("read lease");

    let extended = queue
        .heartbeat(claimed.id, WORKER_A, Duration::from_secs(60))
        .await
        .expect("heartbeat");
    assert!(extended, "owner heartbeat must succeed");

    let new_lease: time::OffsetDateTime =
        sqlx::query_scalar("SELECT locked_until FROM jobs WHERE id = $1")
            .bind(claimed.id)
            .fetch_one(&harness.pool)
            .await
            .expect("read new lease");
    assert!(
        new_lease > original_lease,
        "heartbeat must move locked_until forward"
    );

    let other_extended = queue
        .heartbeat(claimed.id, WORKER_B, Duration::from_secs(60))
        .await
        .expect("heartbeat by other worker");
    assert!(!other_extended, "non-owner heartbeat must return false");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lost_lease_becomes_claimable_by_another_worker() {
    let harness = common::start().await;
    let queue = PostgresJobQueue::<TestPayload>::new(harness.pool.clone());
    let partition = Uuid::now_v7();
    queue
        .enqueue(QUEUE, &[job(partition, "lost", "abandoned")])
        .await
        .expect("enqueue");

    let first = queue
        .claim(QUEUE, WORKER_A, Duration::from_secs(60))
        .await
        .expect("claim 1")
        .expect("a job");
    assert_eq!(first.attempts, 1);

    sqlx::query("UPDATE jobs SET locked_until = NOW() - INTERVAL '1 second' WHERE id = $1")
        .bind(first.id)
        .execute(&harness.pool)
        .await
        .expect("expire lease");

    let second = queue
        .claim(QUEUE, WORKER_B, Duration::from_secs(60))
        .await
        .expect("claim 2")
        .expect("expired lease should be claimable");
    assert_eq!(second.id, first.id);
    assert_eq!(second.attempts, 2, "attempts must increment on re-claim");

    let stale_ack = queue.ack(second.id, WORKER_A).await;
    assert!(stale_ack.is_ok(), "ack returns Ok even when no row deleted");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE id = $1")
        .bind(second.id)
        .fetch_one(&harness.pool)
        .await
        .expect("count");
    assert_eq!(count, 1, "ack by previous owner must not delete the job");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dead_letter_moves_job_to_dead_jobs_table() {
    let harness = common::start().await;
    let queue = PostgresJobQueue::<TestPayload>::new(harness.pool.clone());
    let partition = Uuid::now_v7();
    queue
        .enqueue(QUEUE, &[job(partition, "dead", "doomed")])
        .await
        .expect("enqueue");
    let claimed = queue
        .claim(QUEUE, WORKER_A, Duration::from_secs(30))
        .await
        .expect("claim")
        .expect("a job");

    queue
        .dead_letter(claimed.id, "permanent failure")
        .await
        .expect("dead-letter");

    let live: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE id = $1")
        .bind(claimed.id)
        .fetch_one(&harness.pool)
        .await
        .expect("count live");
    assert_eq!(live, 0);

    let (queue_col, err): (String, Option<String>) =
        sqlx::query_as("SELECT queue, last_error FROM dead_jobs WHERE id = $1")
            .bind(claimed.id)
            .fetch_one(&harness.pool)
            .await
            .expect("read dead_job");
    assert_eq!(queue_col, QUEUE);
    assert_eq!(err.as_deref(), Some("permanent failure"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_partition_deletes_unlocked_jobs_only() {
    let harness = common::start().await;
    let queue = PostgresJobQueue::<TestPayload>::new(harness.pool.clone());
    let partition = Uuid::now_v7();
    let other = Uuid::now_v7();

    queue
        .enqueue(
            QUEUE,
            &[
                job(partition, "p1", "p1"),
                job(partition, "p2", "p2"),
                job(other, "o1", "o1"),
            ],
        )
        .await
        .expect("enqueue");

    let locked = queue
        .claim(QUEUE, WORKER_A, Duration::from_secs(60))
        .await
        .expect("claim")
        .expect("a job");
    assert_eq!(locked.partition_key, Some(partition));

    let cancelled = queue
        .cancel_partition(QUEUE, partition)
        .await
        .expect("cancel partition");
    assert_eq!(cancelled, 1, "only the unlocked partition job is removed");

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE queue = $1 AND partition_key = $2")
            .bind(QUEUE)
            .bind(partition)
            .fetch_one(&harness.pool)
            .await
            .expect("count");
    assert_eq!(remaining, 1, "locked job survives cancel_partition");

    let other_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE queue = $1 AND partition_key = $2")
            .bind(QUEUE)
            .bind(other)
            .fetch_one(&harness.pool)
            .await
            .expect("count other");
    assert_eq!(other_count, 1, "other partitions untouched");
}
