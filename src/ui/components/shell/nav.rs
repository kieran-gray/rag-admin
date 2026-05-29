use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use super::nav_catalog::{chevron_svg, nav_icon, SECTIONS};
use super::theme_toggle::ThemeToggle;
use crate::shared::contracts::ActivityKind;
use crate::ui::state::activity::{set_tray_open, use_activity_state};
use crate::ui::state::event_bus::{use_event_bus, ConnectionState};
use crate::ui::state::sidebar::use_sidebar_state;

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
