use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use crate::ui::state::launcher::use_launcher_state;
use crate::ui::state::recent::title_for_path;

#[derive(Clone)]
struct Crumb {
    label: String,
    href: Option<&'static str>,
}

/// (path, section label, leaf label) for the destinations in the sidebar.
const LEAVES: &[(&str, &str, &str)] = &[
    ("/documents", "Corpus", "Documents"),
    ("/connectors", "Corpus", "Connectors"),
    ("/evaluation/runs", "Evaluation", "Runs"),
    ("/evaluation/datasets", "Evaluation", "Datasets"),
    ("/evaluation/maps", "Evaluation", "Maps"),
    ("/playground/retrieve", "Playground", "Retrieve"),
    ("/playground/embed", "Playground", "Embed"),
    ("/playground/chat", "Playground", "Chat"),
    ("/pipeline/profiles", "Pipeline", "Profiles"),
    ("/pipeline/chunking", "Pipeline", "Chunking"),
    ("/pipeline/catalog", "Pipeline", "Catalog"),
    ("/maintenance/chunk-sets", "Maintenance", "Chunk sets"),
];

fn section(label: &str) -> Crumb {
    Crumb {
        label: label.into(),
        href: None,
    }
}

fn current(label: String) -> Crumb {
    Crumb { label, href: None }
}

fn parent(label: &str, href: &'static str) -> Crumb {
    Crumb {
        label: label.into(),
        href: Some(href),
    }
}

fn crumbs_for_path(path: &str) -> Vec<Crumb> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return vec![current("Home".into())];
    }

    if let Some(&(_, sec, leaf)) = LEAVES.iter().find(|(p, _, _)| *p == trimmed) {
        return vec![section(sec), current(leaf.into())];
    }

    if trimmed.starts_with("/evaluation/runs/") {
        return vec![
            section("Evaluation"),
            parent("Runs", "/evaluation/runs"),
            current(title_for_path(trimmed)),
        ];
    }
    if trimmed.starts_with("/evaluation/datasets/") {
        return vec![
            section("Evaluation"),
            parent("Datasets", "/evaluation/datasets"),
            current(title_for_path(trimmed)),
        ];
    }
    if trimmed.starts_with("/connectors/") {
        return vec![
            section("Corpus"),
            parent("Connectors", "/connectors"),
            current(title_for_path(trimmed)),
        ];
    }
    if trimmed.starts_with("/evaluate/") {
        return vec![section("Evaluation"), current(title_for_path(trimmed))];
    }
    if trimmed.starts_with("/documents/") {
        return vec![
            section("Corpus"),
            parent("Documents", "/documents"),
            current(title_for_path(trimmed)),
        ];
    }

    vec![current(title_for_path(trimmed))]
}

#[component]
pub fn Breadcrumb() -> impl IntoView {
    let location = use_location();
    let crumbs = Signal::derive(move || crumbs_for_path(&location.pathname.get()));
    let launcher = use_launcher_state();

    view! {
        <div class="breadcrumb-bar">
            <div class="breadcrumb-inner max-w-6xl mx-auto px-8">
            <nav class="breadcrumb-trail" aria-label="Breadcrumb">
                {move || {
                    let list = crumbs.get();
                    let last = list.len().saturating_sub(1);
                    list.into_iter().enumerate().map(|(i, crumb)| {
                        let is_last = i == last;
                        let sep = (i > 0).then(|| view! {
                            <span class="breadcrumb-sep" aria-hidden="true">"›"</span>
                        });
                        let node = if is_last {
                            view! {
                                <span class="breadcrumb-current" aria-current="page">
                                    {crumb.label}
                                </span>
                            }.into_any()
                        } else if let Some(href) = crumb.href {
                            view! {
                                <A href=href attr:class="breadcrumb-link">{crumb.label}</A>
                            }.into_any()
                        } else {
                            view! {
                                <span class="breadcrumb-text">{crumb.label}</span>
                            }.into_any()
                        };
                        view! { {sep} {node} }
                    }).collect_view()
                }}
            </nav>
            <button
                type="button"
                class="launcher-trigger"
                aria-label="Open command launcher"
                on:click=move |_| launcher.open()
            >
                <span class="launcher-trigger-icon" aria-hidden="true">"⌕"</span>
                <span class="launcher-trigger-label">"Search"</span>
                <span class="launcher-trigger-kbd" aria-hidden="true">"⌘K"</span>
            </button>
            </div>
        </div>
    }
}
