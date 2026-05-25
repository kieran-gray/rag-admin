use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use leptos_router::{
    components::{Route, Router, Routes},
    NavigateOptions, ParamSegment, StaticSegment,
};

use crate::ui::components::app::event_bus::provide_event_bus;
use crate::ui::components::shell::AppShell;
use crate::ui::pages::{
    artifacts::chunk_sets::ChunkSetsPage,
    configuration::{
        catalog::CatalogPage, chunking::ChunkingPage, connectors::ConnectorsPage,
        profiles::ProfilesPage,
    },
    document_detail::DocumentDetailPage,
    documents::{DocumentByIdRedirect, DocumentsPage},
    evaluate::{EvaluateByIdRedirect, EvaluatePage},
    evaluations::{DatasetDetailPage, DatasetsPage, EvaluationsPage, RunPage},
    playground::{chat::ChatPage, embed::EmbedPage, retrieve::RetrievePage},
};

#[component]
fn RedirectTo(to: &'static str) -> impl IntoView {
    Effect::new(move |_| {
        use_navigate()(
            to,
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });

    view! { <p class="muted">"Redirecting…"</p> }
}

struct LegacyRunTabRedirect;

impl LegacyRunTabRedirect {
    fn progress() -> impl IntoView {
        Self::redirect("progress")
    }

    fn compare() -> impl IntoView {
        Self::redirect("compare")
    }

    fn redirect(tab: &'static str) -> impl IntoView {
        let params = use_params_map();
        let query = use_query_map();
        Effect::new(move |_| {
            let run_id = params.with(|p| p.get("run_id").unwrap_or_default().to_string());
            let with = query.with(|q| q.get("with").map(|t| t.to_string()));
            let target = match with {
                Some(w) => format!("/evaluations/runs/{run_id}?tab={tab}&with={w}"),
                None => format!("/evaluations/runs/{run_id}?tab={tab}"),
            };
            use_navigate()(
                &target,
                NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        });
        view! { <p class="muted">"Redirecting…"</p> }
    }
}

#[component]
fn LegacyRunRedirect() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    Effect::new(move |_| {
        let run_id = params.with(|p| p.get("run_id").unwrap_or_default().to_string());
        let q = query.with(|q| {
            let mut parts: Vec<String> = Vec::new();
            for k in ["tab", "with"] {
                if let Some(v) = q.get(k) {
                    parts.push(format!("{k}={v}"));
                }
            }
            parts.join("&")
        });
        let target = if q.is_empty() {
            format!("/evaluations/runs/{run_id}")
        } else {
            format!("/evaluations/runs/{run_id}?{q}")
        };
        use_navigate()(
            &target,
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });
    view! { <p class="muted">"Redirecting…"</p> }
}

#[component]
fn LegacyDatasetRedirect() -> impl IntoView {
    let params = use_params_map();
    Effect::new(move |_| {
        let dataset_id = params.with(|p| p.get("dataset_id").unwrap_or_default().to_string());
        use_navigate()(
            &format!("/evaluations/datasets/{dataset_id}"),
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });
    view! { <p class="muted">"Redirecting…"</p> }
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <link rel="stylesheet" id="leptos" href="/pkg/rag_admin.css" />
                <link rel="shortcut icon" type="image/ico" href="/favicon.ico" />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_event_bus();

    view! {
        <Title text="rag-admin" />
        <Router>
            <AppShell>
                <Routes fallback=|| view! { <p class="p-8 muted">"Page not found."</p> }>
                    <Route path=StaticSegment("") view=DocumentsPage />
                    <Route
                        path=(
                            StaticSegment("documents"),
                            StaticSegment("by-id"),
                            ParamSegment("document_id"),
                        )
                        view=DocumentByIdRedirect
                    />
                    <Route
                        path=(
                            StaticSegment("documents"),
                            ParamSegment("doc_type"),
                            ParamSegment("source_ref"),
                        )
                        view=DocumentDetailPage
                    />
                    <Route
                        path=(
                            StaticSegment("evaluate"),
                            StaticSegment("by-id"),
                            ParamSegment("document_id"),
                        )
                        view=EvaluateByIdRedirect
                    />
                    <Route
                        path=(
                            StaticSegment("evaluate"),
                            ParamSegment("doc_type"),
                            ParamSegment("source_ref"),
                        )
                        view=EvaluatePage
                    />
                    <Route
                        path=StaticSegment("evaluations")
                        view=|| view! { <RedirectTo to="/evaluations/runs" /> }
                    />
                    <Route
                        path=(StaticSegment("evaluations"), StaticSegment("runs"))
                        view=EvaluationsPage
                    />
                    <Route
                        path=(
                            StaticSegment("evaluations"),
                            StaticSegment("runs"),
                            ParamSegment("run_id"),
                        )
                        view=RunPage
                    />
                    <Route
                        path=(StaticSegment("evaluations"), StaticSegment("datasets"))
                        view=DatasetsPage
                    />
                    <Route
                        path=(
                            StaticSegment("evaluations"),
                            StaticSegment("datasets"),
                            ParamSegment("dataset_id"),
                        )
                        view=DatasetDetailPage
                    />
                    <Route
                        path=(StaticSegment("runs"), ParamSegment("run_id"))
                        view=LegacyRunRedirect
                    />
                    <Route
                        path=(
                            StaticSegment("runs"),
                            ParamSegment("run_id"),
                            StaticSegment("optimize"),
                        )
                        view=LegacyRunTabRedirect::progress
                    />
                    <Route
                        path=(
                            StaticSegment("runs"),
                            ParamSegment("run_id"),
                            StaticSegment("replicate"),
                        )
                        view=LegacyRunTabRedirect::compare
                    />
                    <Route
                        path=(StaticSegment("datasets"), ParamSegment("dataset_id"))
                        view=LegacyDatasetRedirect
                    />
                    <Route
                        path=(StaticSegment("configuration"), StaticSegment("catalog"))
                        view=CatalogPage
                    />
                    <Route
                        path=(StaticSegment("configuration"), StaticSegment("profiles"))
                        view=ProfilesPage
                    />
                    <Route
                        path=(StaticSegment("configuration"), StaticSegment("index-profiles"))
                        view=|| view! { <RedirectTo to="/configuration/profiles" /> }
                    />
                    <Route
                        path=(StaticSegment("configuration"), StaticSegment("retrieval-profiles"))
                        view=|| view! { <RedirectTo to="/configuration/profiles" /> }
                    />
                    <Route
                        path=(StaticSegment("configuration"), StaticSegment("chunking"))
                        view=ChunkingPage
                    />
                    <Route
                        path=(StaticSegment("configuration"), StaticSegment("connectors"))
                        view=ConnectorsPage
                    />
                    <Route
                        path=(StaticSegment("artifacts"), StaticSegment("chunk-sets"))
                        view=ChunkSetsPage
                    />
                    <Route
                        path=StaticSegment("artifacts")
                        view=|| view! { <RedirectTo to="/artifacts/chunk-sets" /> }
                    />
                    <Route
                        path=StaticSegment("configuration")
                        view=|| view! { <RedirectTo to="/configuration/catalog" /> }
                    />
                    <Route
                        path=(StaticSegment("playground"), StaticSegment("embed"))
                        view=EmbedPage
                    />
                    <Route
                        path=(StaticSegment("playground"), StaticSegment("retrieve"))
                        view=RetrievePage
                    />
                    <Route
                        path=(StaticSegment("playground"), StaticSegment("chat"))
                        view=ChatPage
                    />
                    <Route
                        path=StaticSegment("playground")
                        view=|| view! { <RedirectTo to="/playground/retrieve" /> }
                    />
                    <Route
                        path=StaticSegment("settings")
                        view=|| view! { <RedirectTo to="/configuration/catalog" /> }
                    />
                    <Route
                        path=StaticSegment("pipelines")
                        view=|| view! { <RedirectTo to="/configuration/profiles" /> }
                    />
                    <Route
                        path=StaticSegment("chunking")
                        view=|| view! { <RedirectTo to="/configuration/chunking" /> }
                    />
                    <Route
                        path=StaticSegment("embed")
                        view=|| view! { <RedirectTo to="/playground/embed" /> }
                    />
                </Routes>
            </AppShell>
        </Router>
    }
}
