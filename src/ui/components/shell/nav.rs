use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use super::theme_toggle::ThemeToggle;
use crate::shared::contracts::ActivityKind;
use crate::ui::state::activity::{set_tray_open, use_activity_state};
use crate::ui::state::event_bus::{use_event_bus, ConnectionState};
use crate::ui::state::sidebar::use_sidebar_state;

#[derive(Clone, Copy)]
struct NavItem {
    href: &'static str,
    label: &'static str,
    hint: &'static str,
    icon: NavIcon,
    watch: Option<ActivityKind>,
}

#[derive(Clone, Copy)]
struct NavSection {
    label: Option<&'static str>,
    items: &'static [NavItem],
}

#[derive(Clone, Copy)]
enum NavIcon {
    Home,
    Documents,
    Connectors,
    Runs,
    Datasets,
    Maps,
    Retrieve,
    Embed,
    Chat,
    Profiles,
    Chunking,
    Catalog,
    ChunkSets,
}

const SECTIONS: &[NavSection] = &[
    NavSection {
        label: None,
        items: &[NavItem {
            href: "/",
            label: "Home",
            hint: "",
            icon: NavIcon::Home,
            watch: None,
        }],
    },
    NavSection {
        label: Some("Corpus"),
        items: &[
            NavItem {
                href: "/documents",
                label: "Documents",
                hint: "add & index",
                icon: NavIcon::Documents,
                watch: Some(ActivityKind::Indexing),
            },
            NavItem {
                href: "/connectors",
                label: "Connectors",
                hint: "ingest sources",
                icon: NavIcon::Connectors,
                watch: None,
            },
        ],
    },
    NavSection {
        label: Some("Evaluation"),
        items: &[
            NavItem {
                href: "/evaluation/runs",
                label: "Runs",
                hint: "recall@k across variants",
                icon: NavIcon::Runs,
                watch: Some(ActivityKind::EvaluationRun),
            },
            NavItem {
                href: "/evaluation/datasets",
                label: "Datasets",
                hint: "question sets",
                icon: NavIcon::Datasets,
                watch: Some(ActivityKind::EvaluationDataset),
            },
            NavItem {
                href: "/evaluation/maps",
                label: "Maps",
                hint: "comprehension",
                icon: NavIcon::Maps,
                watch: Some(ActivityKind::DocumentMap),
            },
        ],
    },
    NavSection {
        label: Some("Playground"),
        items: &[
            NavItem {
                href: "/playground/retrieve",
                label: "Retrieve",
                hint: "top-k search",
                icon: NavIcon::Retrieve,
                watch: None,
            },
            NavItem {
                href: "/playground/embed",
                label: "Embed",
                hint: "compare vectors",
                icon: NavIcon::Embed,
                watch: None,
            },
            NavItem {
                href: "/playground/chat",
                label: "Chat",
                hint: "grounded answer",
                icon: NavIcon::Chat,
                watch: None,
            },
        ],
    },
    NavSection {
        label: Some("Pipeline"),
        items: &[
            NavItem {
                href: "/pipeline/profiles",
                label: "Profiles",
                hint: "index + retrieval",
                icon: NavIcon::Profiles,
                watch: None,
            },
            NavItem {
                href: "/pipeline/chunking",
                label: "Chunking",
                hint: "strategies · sweeps",
                icon: NavIcon::Chunking,
                watch: None,
            },
            NavItem {
                href: "/pipeline/catalog",
                label: "Catalog",
                hint: "models · indexes",
                icon: NavIcon::Catalog,
                watch: None,
            },
        ],
    },
    NavSection {
        label: Some("Maintenance"),
        items: &[NavItem {
            href: "/maintenance/chunk-sets",
            label: "Chunk sets",
            hint: "cached chunks · GC",
            icon: NavIcon::ChunkSets,
            watch: None,
        }],
    },
];

#[component]
pub fn AppSidebar() -> impl IntoView {
    let location = use_location();
    let bus = use_event_bus();
    let activity = use_activity_state();
    let sidebar = use_sidebar_state();
    let tray_open = activity.open;
    let collapsed = sidebar.collapsed;

    let is_active = move |href: &'static str| {
        let path = location.pathname.get();
        if href == "/" {
            path == "/"
        } else {
            path == href || path.starts_with(&format!("{href}/"))
        }
    };

    let running_kinds = Signal::derive(move || {
        activity
            .running_jobs()
            .into_iter()
            .map(|j| j.kind)
            .collect::<Vec<_>>()
    });
    let kind_running = move |kind: ActivityKind| running_kinds.with(|k| k.contains(&kind));

    let connection_label = move || match bus.connection.get() {
        ConnectionState::Open => "Local · connected",
        ConnectionState::Connecting => "Local · connecting…",
        ConnectionState::Closed => "Local · disconnected",
    };

    let connection_dot_class = move || match bus.connection.get() {
        ConnectionState::Open => "activity-dot",
        ConnectionState::Connecting => "activity-dot activity-dot-active",
        ConnectionState::Closed => "activity-dot activity-dot-fail",
    };

    let tray_btn_class = move || {
        let base = "btn-ghost btn nav-tray-btn";
        if tray_open.get() {
            format!("{base} is-active")
        } else {
            base.to_string()
        }
    };

    let collapse_label = move || {
        if collapsed.get() {
            "Expand sidebar"
        } else {
            "Collapse sidebar"
        }
    };

    view! {
        <aside class="app-sidebar">
            <div class="app-sidebar-head">
                <A href="/" attr:class="wordmark app-sidebar-brand" attr:aria-label="rag-admin home">
                    <span class="wordmark-text">"rag-admin"</span>
                </A>
                <button
                    type="button"
                    class="sidebar-collapse-btn"
                    title=collapse_label
                    aria-label=collapse_label
                    aria-expanded=move || if collapsed.get() { "false" } else { "true" }
                    on:click=move |_| sidebar.toggle()
                >
                    <span
                        class=move || if collapsed.get() {
                            "sidebar-collapse-glyph is-collapsed"
                        } else {
                            "sidebar-collapse-glyph"
                        }
                    >
                        {chevron_svg()}
                    </span>
                </button>
            </div>

            <nav class="app-sidebar-nav" aria-label="Primary">
                {SECTIONS.iter().map(|section| {
                    let items = section.items;
                    view! {
                        <div class="nav-section">
                            {section.label.map(|label| view! {
                                <div class="nav-section-label">{label}</div>
                            })}
                            {items.iter().map(|item| {
                                let href = item.href;
                                let watch = item.watch;
                                let active = move || is_active(href);
                                let link_class = move || if active() {
                                    "nav-link is-active"
                                } else {
                                    "nav-link"
                                };
                                let live = move || watch.map(kind_running).unwrap_or(false);
                                view! {
                                    <A
                                        href=href
                                        attr:class=link_class
                                        attr:title=item.label
                                        attr:aria-current=move || if active() { "page" } else { "" }
                                    >
                                        <span class="nav-link-icon" aria-hidden="true">
                                            {nav_icon(item.icon)}
                                        </span>
                                        <span class="nav-link-text">
                                            <span class="nav-link-label">{item.label}</span>
                                            {(!item.hint.is_empty()).then(|| view! {
                                                <span class="nav-link-hint">{item.hint}</span>
                                            })}
                                        </span>
                                        {move || live().then(|| view! {
                                            <span
                                                class="activity-dot activity-dot-active nav-link-live"
                                                title="Running"
                                                aria-label="Running"
                                            ></span>
                                        })}
                                    </A>
                                }
                            }).collect_view()}
                        </div>
                    }
                }).collect_view()}
            </nav>

            <div class="app-sidebar-footer">
                <span class="app-sidebar-local" title=connection_label aria-label=connection_label>
                    <span class=connection_dot_class></span>
                    <span class="app-sidebar-local-label">"Local"</span>
                </span>
                <span class="toolbar-spacer"></span>
                <ThemeToggle />
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
        </aside>
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

fn chevron_svg() -> AnyView {
    view! {
        <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            <path d="m15 18-6-6 6-6" />
        </svg>
    }
    .into_any()
}

fn icon_svg(children: AnyView) -> AnyView {
    view! {
        <svg
            class="nav-icon-svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.7"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            {children}
        </svg>
    }
    .into_any()
}

fn nav_icon(icon: NavIcon) -> AnyView {
    match icon {
        NavIcon::Home => icon_svg(
            view! {
                <path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
                <path d="M9 22V12h6v10" />
            }
            .into_any(),
        ),
        NavIcon::Documents => icon_svg(
            view! {
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                <path d="M14 2v6h6" />
                <path d="M16 13H8" />
                <path d="M16 17H8" />
            }
            .into_any(),
        ),
        NavIcon::Connectors => icon_svg(
            view! {
                <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
                <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
            }
            .into_any(),
        ),
        NavIcon::Runs => icon_svg(
            view! {
                <path d="M3 3v18h18" />
                <path d="M18 17V9" />
                <path d="M13 17V5" />
                <path d="M8 17v-3" />
            }
            .into_any(),
        ),
        NavIcon::Datasets => icon_svg(
            view! {
                <ellipse cx="12" cy="5" rx="9" ry="3" />
                <path d="M3 5v14a9 3 0 0 0 18 0V5" />
                <path d="M3 12a9 3 0 0 0 18 0" />
            }
            .into_any(),
        ),
        NavIcon::Maps => icon_svg(
            view! {
                <path d="M9 3 3 6v15l6-3 6 3 6-3V3l-6 3z" />
                <path d="M9 3v15" />
                <path d="M15 6v15" />
            }
            .into_any(),
        ),
        NavIcon::Retrieve => icon_svg(
            view! {
                <circle cx="11" cy="11" r="7" />
                <path d="m21 21-4.3-4.3" />
            }
            .into_any(),
        ),
        NavIcon::Embed => icon_svg(
            view! {
                <circle cx="9" cy="12" r="6" />
                <circle cx="15" cy="12" r="6" />
            }
            .into_any(),
        ),
        NavIcon::Chat => icon_svg(
            view! {
                <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            }
            .into_any(),
        ),
        NavIcon::Profiles => icon_svg(
            view! {
                <path d="M4 21v-7" />
                <path d="M4 10V3" />
                <path d="M12 21v-9" />
                <path d="M12 8V3" />
                <path d="M20 21v-5" />
                <path d="M20 12V3" />
                <path d="M1 14h6" />
                <path d="M9 8h6" />
                <path d="M17 16h6" />
            }
            .into_any(),
        ),
        NavIcon::Chunking => icon_svg(
            view! {
                <circle cx="6" cy="6" r="3" />
                <circle cx="6" cy="18" r="3" />
                <path d="M20 4 8.12 15.88" />
                <path d="m14.47 14.48 5.53 5.52" />
                <path d="M8.12 8.12 12 12" />
            }
            .into_any(),
        ),
        NavIcon::Catalog => icon_svg(
            view! {
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            }
            .into_any(),
        ),
        NavIcon::ChunkSets => icon_svg(
            view! {
                <rect x="2" y="3" width="20" height="5" rx="1" />
                <path d="M4 8v11a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1V8" />
                <path d="M10 12h4" />
            }
            .into_any(),
        ),
    }
}
