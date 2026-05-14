use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    components::{Route, Router, Routes},
    ParamSegment, StaticSegment,
};

use crate::components::event_bus::provide_event_bus;
use crate::components::shell::AppShell;
use crate::pages::{
    chunking::ChunkingPage,
    document_detail::{DatasetDetailPage, DocumentByIdRedirect, DocumentDetailPage, RunDetailPage},
    embed_test::EmbedTestPage,
    evaluations::EvaluationsPage,
    pipelines::PipelinesPage,
    playground::PlaygroundPage,
    posts_list::PostsListPage,
    settings::SettingsPage,
};

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
                    <Route path=StaticSegment("") view=PostsListPage />
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
                    <Route path=StaticSegment("evaluations") view=EvaluationsPage />
                    <Route
                        path=(StaticSegment("runs"), ParamSegment("run_id"))
                        view=RunDetailPage
                    />
                    <Route
                        path=(StaticSegment("datasets"), ParamSegment("dataset_id"))
                        view=DatasetDetailPage
                    />
                    <Route path=StaticSegment("pipelines") view=PipelinesPage />
                    <Route path=StaticSegment("chunking") view=ChunkingPage />
                    <Route path=StaticSegment("playground") view=PlaygroundPage />
                    <Route path=StaticSegment("settings") view=SettingsPage />
                    <Route path=StaticSegment("embed") view=EmbedTestPage />
                </Routes>
            </AppShell>
        </Router>
    }
}
