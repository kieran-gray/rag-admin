#[cfg(feature = "ssr")]
#[tokio::main]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "main: startup failures cannot be handled, so panicking is the appropriate response"
)]
async fn main() {
    use axum::extract::DefaultBodyLimit;
    use axum::routing::{get, post};
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use search_crucible::app::{shell, App as LeptosApp};
    use search_crucible::server::api::chat_stream::chat_stream_handler;
    use search_crucible::server::api::events_ws::events_ws_handler;
    use search_crucible::server::api::health::health_check;
    use search_crucible::server::api::sse::job_logs_handler;
    use search_crucible::server::api::upload::upload_document;
    use search_crucible::server::setup::bootstrap;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(LeptosApp);

    let app = Arc::new(bootstrap().await.expect("failed to bootstrap application"));

    let router = Router::new()
        .route("/api/job/logs/{job_id}", get(job_logs_handler))
        .route("/api/events/ws", get(events_ws_handler))
        .route("/api/health", get(health_check))
        .route("/api/chat_stream", post(chat_stream_handler))
        .route(
            "/api/source_documents/upload",
            post(upload_document).layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            {
                let app = Arc::clone(&app);
                move || app.provide_contexts()
            },
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    let router = app.apply_axum_extensions(router);

    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!("search-crucible listening on http://{}", &addr);
    axum::serve(listener, router.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
