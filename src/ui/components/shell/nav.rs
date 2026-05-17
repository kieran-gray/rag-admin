use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;
use uuid::Uuid;

use super::nav_menu::{NavMenu, NavMenuItem};
use crate::contracts::{ActivityJobDto, ActivityKind};
use crate::ui::components::activity::{open_tray_with, set_tray_open, use_activity_state};
use crate::ui::components::event_bus::{use_event_bus, ConnectionState};

const CONFIGURATION_ITEMS: &[NavMenuItem] = &[
    NavMenuItem {
        href: "/configuration/catalog",
        label: "Catalog",
        hint: "models · indexes",
    },
    NavMenuItem {
        href: "/configuration/pipelines",
        label: "Pipelines",
        hint: "compose models + index",
    },
    NavMenuItem {
        href: "/configuration/chunking",
        label: "Chunking",
        hint: "strategies · sweeps",
    },
];

const PLAYGROUND_ITEMS: &[NavMenuItem] = &[
    NavMenuItem {
        href: "/playground/embed",
        label: "Embed",
        hint: "compare vectors",
    },
    NavMenuItem {
        href: "/playground/retrieve",
        label: "Retrieve",
        hint: "top-K search",
    },
];

#[component]
pub fn AppNav() -> impl IntoView {
    let location = use_location();
    let bus = use_event_bus();
    let activity = use_activity_state();
    let tray_open = activity.open;

    let is_active = move |href: &'static str| {
        let path = location.pathname.get();
        if href == "/" {
            path == "/" || path.starts_with("/documents")
        } else {
            path == href || path.starts_with(&format!("{href}/"))
        }
    };

    let running = Signal::derive(move || activity.running_jobs());

    let connection_label = move || match bus.connection.get() {
        ConnectionState::Open => "Connected",
        ConnectionState::Connecting => "Connecting…",
        ConnectionState::Closed => "Disconnected",
    };

    let connection_dot_class = move || match bus.connection.get() {
        ConnectionState::Open => "activity-dot",
        ConnectionState::Connecting => "activity-dot activity-dot-active",
        ConnectionState::Closed => "activity-dot activity-dot-fail",
    };

    let tray_btn_class = move || {
        let base = "btn-ghost btn flex items-center gap-2 nav-tray-btn";
        if tray_open.get() {
            format!("{base} is-active")
        } else {
            base.to_string()
        }
    };

    view! {
        <header class="app-header">
            <div class="max-w-6xl mx-auto px-6 py-3 flex items-center gap-6">
                <A href="/" attr:class="wordmark" attr:aria-label="rag-admin home">
                    "rag-admin"
                </A>
                <nav class="flex flex-grow justify-around gap-5 text-sm items-center">
                    {[("/", "Documents"), ("/evaluations", "Evaluations")].iter().copied().map(|(href, label)| {
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
                    <NavMenu label="Playground" prefix="/playground" items=PLAYGROUND_ITEMS />
                    <NavMenu label="Configuration" prefix="/configuration" items=CONFIGURATION_ITEMS />
                </nav>

                <RunningCount running=running />

                <span
                    class=connection_dot_class
                    title=connection_label
                    aria-label=connection_label
                ></span>

                <button
                    type="button"
                    class=tray_btn_class
                    title="Toggle activity tray (Ctrl+`)"
                    aria-label="Toggle activity tray"
                    aria-expanded=move || if tray_open.get() { "true" } else { "false" }
                    on:click=move |_| set_tray_open(!tray_open.get_untracked())
                >
                    <TrayGlyph open=tray_open />
                </button>
            </div>
        </header>
    }
}

#[component]
fn RunningCount(#[prop(into)] running: Signal<Vec<ActivityJobDto>>) -> impl IntoView {
    let (open, set_open) = signal(false);

    #[cfg(feature = "hydrate")]
    let outside_listener = StoredValue::new_local(None::<self::hydrate::OutsideClick>);

    let toggle = move |_| {
        let next = !open.get_untracked();
        set_open.set(next);
        #[cfg(feature = "hydrate")]
        {
            if next {
                outside_listener.set_value(Some(self::hydrate::watch_outside_clicks(set_open)));
            } else {
                outside_listener.set_value(None);
            }
        }
    };

    let close_dropdown = move || {
        set_open.set(false);
        #[cfg(feature = "hydrate")]
        outside_listener.set_value(None);
    };

    view! {
        {move || {
            let list = running.get();
            if list.is_empty() {
                return view! { <span></span> }.into_any();
            }
            let total = list.len();
            view! {
                <div class="nav-running" data-running=move || open.get().to_string()>
                    <button
                        type="button"
                        class="nav-chip nav-chip-stack"
                        title=format!("{total} active job{}", if total == 1 { "" } else { "s" })
                        aria-haspopup="true"
                        aria-expanded=move || if open.get() { "true" } else { "false" }
                        on:click=toggle
                    >
                        <span class="activity-dot activity-dot-active"></span>
                        <span class="nav-chip-label">
                            {format!("{total} active")}
                        </span>
                        <span class="nav-chip-caret" aria-hidden="true">"▾"</span>
                    </button>
                    {move || open.get().then(|| {
                        let entries = list.clone();
                        view! {
                            <div class="nav-running-menu" role="menu">
                                <div class="nav-running-menu-header">"Running"</div>
                                {entries.into_iter().map(|job| {
                                    let id = job.stream_id;
                                    let kind = kind_label(job.kind);
                                    let short = short_id(id);
                                    let label = job.label.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="nav-running-menu-item"
                                            role="menuitem"
                                            on:click=move |_| {
                                                open_tray_with(id);
                                                close_dropdown();
                                            }
                                        >
                                            <span class="activity-dot activity-dot-active"></span>
                                            <div class="nav-running-menu-text">
                                                <span class="nav-running-menu-label">{label}</span>
                                                <span class="nav-running-menu-meta">
                                                    {kind}" · "{short}
                                                </span>
                                            </div>
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                        }
                    })}
                </div>
            }.into_any()
        }}
    }
}

fn kind_label(kind: ActivityKind) -> &'static str {
    match kind {
        ActivityKind::Indexing => "Indexing",
        ActivityKind::EvaluationDataset => "Dataset",
        ActivityKind::EvaluationRun => "Run",
    }
}

#[component]
fn TrayGlyph(open: RwSignal<bool>) -> impl IntoView {
    view! {
        <span class="nav-tray-glyph" aria-hidden="true">
            <span class="nav-tray-glyph-top"></span>
            <span class=move || if open.get() {
                "nav-tray-glyph-bottom is-open"
            } else {
                "nav-tray-glyph-bottom"
            }></span>
        </span>
    }
}

fn short_id(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

#[cfg(feature = "hydrate")]
mod hydrate {
    use leptos::prelude::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    pub struct OutsideClick {
        closure: Option<Closure<dyn FnMut(web_sys::MouseEvent)>>,
    }

    impl Drop for OutsideClick {
        fn drop(&mut self) {
            if let (Some(window), Some(c)) = (web_sys::window(), self.closure.take()) {
                _ = window
                    .remove_event_listener_with_callback("mousedown", c.as_ref().unchecked_ref());
            }
        }
    }

    pub fn watch_outside_clicks(set_open: WriteSignal<bool>) -> OutsideClick {
        let Some(window) = web_sys::window() else {
            return OutsideClick { closure: None };
        };
        let closure =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |ev: web_sys::MouseEvent| {
                let target = ev
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
                if let Some(el) = target {
                    if el.closest(".nav-running").ok().flatten().is_some() {
                        return;
                    }
                }
                set_open.set(false);
            });
        _ = window.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref());
        OutsideClick {
            closure: Some(closure),
        }
    }
}
