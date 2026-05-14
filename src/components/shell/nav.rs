use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use crate::components::activity::{toggle_drawer, use_activity_state};
use crate::components::event_bus::{use_event_bus, ConnectionState};

#[component]
pub fn AppNav() -> impl IntoView {
    let location = use_location();
    let bus = use_event_bus();
    let activity = use_activity_state();

    let destinations: &[(&'static str, &'static str)] = &[
        ("/", "Documents"),
        ("/evaluations", "Evaluations"),
        ("/pipelines", "Pipelines"),
        ("/chunking", "Chunking"),
        ("/playground", "Playground"),
    ];

    let is_active = move |href: &'static str| {
        let path = location.pathname.get();
        if href == "/" {
            path == "/" || path.starts_with("/documents")
        } else {
            path == href || path.starts_with(&format!("{href}/"))
        }
    };

    let running_count = move || activity.running_count();
    let drawer_open = activity.open;

    let activity_dot_class = move || {
        let running = running_count() > 0;
        let connected = matches!(bus.connection.get(), ConnectionState::Open);
        match (connected, running) {
            (false, _) => "activity-dot activity-dot-fail",
            (true, false) => "activity-dot",
            (true, true) => "activity-dot activity-dot-active",
        }
    };

    let activity_title = move || match (bus.connection.get(), running_count()) {
        (ConnectionState::Open, 0) => "Connected · idle".to_string(),
        (ConnectionState::Open, n) => format!("Connected · {n} active job(s)"),
        (ConnectionState::Connecting, _) => "Connecting…".to_string(),
        (ConnectionState::Closed, _) => "Disconnected".to_string(),
    };

    view! {
        <header class="app-header">
            <div class="max-w-6xl mx-auto px-6 py-3 flex items-center gap-8">
                <A href="/" attr:class="wordmark" attr:aria-label="rag-admin home">
                    "rag-admin"
                </A>
                <nav class="flex gap-5 text-sm">
                    {destinations.iter().copied().map(|(href, label)| {
                        let active = move || is_active(href);
                        view! {
                            <A
                                href=href
                                attr:class="app-nav-link"
                                attr:aria-current=move || if active() { "page" } else { "" }
                            >
                                {label}
                            </A>
                        }
                    }).collect_view()}
                </nav>
                <div class="toolbar-spacer"></div>
                <button
                    type="button"
                    class="btn-ghost btn flex items-center gap-2"
                    title=activity_title
                    aria-label="Toggle activity drawer"
                    aria-expanded=move || if drawer_open.get() { "true" } else { "false" }
                    on:click=move |_| toggle_drawer(!drawer_open.get_untracked())
                >
                    <span class=activity_dot_class></span>
                    <span class="muted text-xs">
                        {move || running_count().to_string()}
                    </span>
                </button>
                <A href="/settings" attr:class="btn-ghost btn" attr:title="Settings">
                    "Settings"
                </A>
            </div>
        </header>
    }
}
