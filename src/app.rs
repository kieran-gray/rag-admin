use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::hooks::use_navigate;
use leptos_router::{
    components::{Route, Router, Routes},
    NavigateOptions, ParamSegment, StaticSegment,
};

use crate::ui::components::playground::provide_playground_context;
use crate::ui::components::shell::AppShell;
use crate::ui::pages::{
    artifacts::{chunk_sets::ChunkSetsPage, maps::MapsPage, DatasetDetailPage, DatasetsPage},
    configuration::{
        catalog::CatalogPage, chunking::ChunkingPage, connectors::ConnectorsPage,
        profiles::ProfilesPage,
    },
    docs::DocsPage,
    document_detail::{DocumentDetailPage, DocumentIngestPage, DocumentMapPage},
    documents::{ConnectorPullPage, DocumentByIdRedirect, DocumentsPage},
    evaluate::{EvaluateByIdRedirect, EvaluatePage},
    home::HomePage,
    playground::{chat::ChatPage, embed::EmbedPage, retrieve::RetrievePage},
    workflows::{EvaluateWorkflowPage, IngestWorkflowPage, RunPage, RunsPage},
};
use crate::ui::state::event_bus::provide_event_bus;

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

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <script inner_html=THEME_BOOT_SCRIPT />
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

const THEME_BOOT_SCRIPT: &str = "(function(){try{var s=localStorage.getItem('rag-admin-theme');var t=s?s:(window.matchMedia&&window.matchMedia('(prefers-color-scheme: light)').matches?'light':'dark');if(t==='light'){document.documentElement.setAttribute('data-theme','light');}}catch(e){}})();";

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_event_bus();
    provide_playground_context();

    view! {
        <Title text="rag-admin" />
        <Router>
            <AppShell>
                <Routes fallback=|| view! { <p class="p-8 muted">"Page not found."</p> }>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=StaticSegment("documents") view=DocumentsPage />
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
                            StaticSegment("documents"),
                            ParamSegment("doc_type"),
                            ParamSegment("source_ref"),
                            StaticSegment("ingest"),
                        )
                        view=DocumentIngestPage
                    />
                    <Route
                        path=(
                            StaticSegment("documents"),
                            ParamSegment("doc_type"),
                            ParamSegment("source_ref"),
                            StaticSegment("map"),
                        )
                        view=DocumentMapPage
                    />
                    <Route path=StaticSegment("connectors") view=ConnectorsPage />
                    <Route
                        path=(StaticSegment("connectors"), ParamSegment("connector_id"))
                        view=ConnectorPullPage
                    />
                    <Route
                        path=(StaticSegment("evaluation"), StaticSegment("runs"))
                        view=RunsPage
                    />
                    <Route
                        path=(
                            StaticSegment("evaluation"),
                            StaticSegment("runs"),
                            ParamSegment("run_id"),
                        )
                        view=RunPage
                    />
                    <Route
                        path=(StaticSegment("evaluation"), StaticSegment("datasets"))
                        view=DatasetsPage
                    />
                    <Route
                        path=(
                            StaticSegment("evaluation"),
                            StaticSegment("datasets"),
                            ParamSegment("dataset_id"),
                        )
                        view=DatasetDetailPage
                    />
                    <Route
                        path=(StaticSegment("evaluation"), StaticSegment("maps"))
                        view=MapsPage
                    />
                    <Route
                        path=StaticSegment("evaluation")
                        view=|| view! { <RedirectTo to="/evaluation/runs" /> }
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
                        path=(StaticSegment("pipeline"), StaticSegment("profiles"))
                        view=ProfilesPage
                    />
                    <Route
                        path=(StaticSegment("pipeline"), StaticSegment("chunking"))
                        view=ChunkingPage
                    />
                    <Route
                        path=(StaticSegment("pipeline"), StaticSegment("catalog"))
                        view=CatalogPage
                    />
                    <Route
                        path=StaticSegment("pipeline")
                        view=|| view! { <RedirectTo to="/pipeline/profiles" /> }
                    />
                    <Route
                        path=(StaticSegment("maintenance"), StaticSegment("chunk-sets"))
                        view=ChunkSetsPage
                    />
                    <Route
                        path=StaticSegment("maintenance")
                        view=|| view! { <RedirectTo to="/maintenance/chunk-sets" /> }
                    />
                    <Route
                        path=(StaticSegment("workflows"), StaticSegment("ingest"))
                        view=IngestWorkflowPage
                    />
                    <Route
                        path=(StaticSegment("workflows"), StaticSegment("evaluate"))
                        view=EvaluateWorkflowPage
                    />
                    <Route path=StaticSegment("docs") view=DocsPage />
                    <Route
                        path=(StaticSegment("docs"), ParamSegment("slug"))
                        view=DocsPage
                    />
                </Routes>
            </AppShell>
        </Router>
    }
}
