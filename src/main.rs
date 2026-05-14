#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use rag_admin::app::{shell, App as LeptosApp};
    use rag_admin::server::api::events_ws::events_ws_handler;
    use rag_admin::server::api::health::health_check;
    use rag_admin::server::api::sse::job_logs_handler;
    use rag_admin::server::setup::bootstrap;
    use std::sync::Arc;
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
        .route(
            "/api/job/logs/{job_id}",
            axum::routing::get(job_logs_handler),
        )
        .route("/api/events/ws", axum::routing::get(events_ws_handler))
        .route("/api/health", axum::routing::get(health_check))
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

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("rag-admin listening on http://{}", &addr);
    axum::serve(listener, router.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
