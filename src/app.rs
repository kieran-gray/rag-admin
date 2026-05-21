use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::hooks::use_navigate;
use leptos_router::{
    components::{Route, Router, Routes},
    NavigateOptions, ParamSegment, StaticSegment,
};

use crate::ui::components::app::event_bus::provide_event_bus;
use crate::ui::components::shell::AppShell;
use crate::ui::pages::{
    configuration::{
        catalog::CatalogPage, chunking::ChunkingPage, connectors::ConnectorsPage,
        profiles::ProfilesPage,
    },
    document_detail::DocumentDetailPage,
    documents::{DocumentByIdRedirect, DocumentsPage},
    evaluate::{EvaluateByIdRedirect, EvaluatePage},
    evaluations::{
        DatasetDetailPage, EvaluationsPage, OptimizeProgressPage, ReplicateComparePage,
        RunDetailPage,
    },
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
                    <Route path=StaticSegment("evaluations") view=EvaluationsPage />
                    <Route
                        path=(StaticSegment("runs"), ParamSegment("run_id"))
                        view=RunDetailPage
                    />
                    <Route
                        path=(
                            StaticSegment("runs"),
                            ParamSegment("run_id"),
                            StaticSegment("optimize"),
                        )
                        view=OptimizeProgressPage
                    />
                    <Route
                        path=(
                            StaticSegment("runs"),
                            ParamSegment("run_id"),
                            StaticSegment("replicate"),
                        )
                        view=ReplicateComparePage
                    />
                    <Route
                        path=(StaticSegment("datasets"), ParamSegment("dataset_id"))
                        view=DatasetDetailPage
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
