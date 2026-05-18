#![cfg(feature = "ssr")]
#![allow(dead_code)]

use std::sync::OnceLock;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::sync::Mutex;

pub struct PgHarness {
    _container: ContainerAsync<Postgres>,
    pub pool: PgPool,
    pub url: String,
}

static SHARED: OnceLock<Mutex<()>> = OnceLock::new();

pub async fn start() -> PgHarness {
    let container = Postgres::default()
        .with_name("pgvector/pgvector")
        .with_tag("pg16")
        .start()
        .await
        .expect("start postgres container");

    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432.tcp())
        .await
        .expect("container port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&url)
        .await
        .expect("connect to test postgres");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("apply migrations");

    PgHarness {
        _container: container,
        pool,
        url,
    }
}
