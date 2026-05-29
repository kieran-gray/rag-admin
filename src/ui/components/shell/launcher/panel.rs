#[cfg(feature = "hydrate")]
use gloo_timers::future::TimeoutFuture;
use std::collections::HashSet;

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use super::command::build_commands;
use super::entities::{
    connector_entry, dataset_entry, document_entry, load_catalog, run_entry, search_documents,
    EntityCatalog,
};
use super::item::{render_icon, LauncherAction, LauncherEntry, LauncherGroup, LauncherIcon};
use super::matcher::score;
use crate::shared::contracts::DocumentListItemDto;
use crate::ui::components::primitives::StatusPill;
use crate::ui::components::shell::nav_catalog::SECTIONS;
use crate::ui::state::launcher::use_launcher_state;
use crate::ui::state::recent::recent_locations;

const MAX_RESULTS: usize = 40;
#[cfg(feature = "hydrate")]
const DEBOUNCE_MS: u32 = 160;

#[component]
pub fn CommandLauncher() -> impl IntoView {
    let launcher = use_launcher_state();
    let open = launcher.open;

    let query = RwSignal::new(String::new());
    let selected = RwSignal::new(0usize);
    let debounced = RwSignal::new(String::new());
    let input_ref = NodeRef::<html::Input>::new();

    let parsed = Memo::new(move |_| parse_query(&query.get()));
    let command_mode = Memo::new(move |_| parsed.get().0);
    let term = Memo::new(move |_| parsed.get().1);
    let nav_term = Memo::new(move |_| {
        let (cmd, t) = parsed.get();
        if cmd {
            String::new()
        } else {
            t
        }
    });

    let docs = Resource::new(
        move || (open.get(), debounced.get()),
        |(is_open, q)| async move {
            if !is_open {
                return Vec::<DocumentListItemDto>::new();
            }
            search_documents(q).await
        },
    );
    let catalog = Resource::new(
        move || open.get(),
        |is_open| async move {
            if !is_open {
                return EntityCatalog::default();
            }
            load_catalog().await
        },
    );

    let commands = StoredValue::new(build_commands());

    let results = Signal::derive(move || {
        let cmd_mode = command_mode.get();
        let term_lower = term.get().to_lowercase();
        let mut scored: Vec<(u8, i32, LauncherEntry)> = Vec::new();

        if cmd_mode {
            for entry in commands.get_value() {
                if let Some(s) = score(&entry.haystack, &term_lower) {
                    scored.push((entry.group.order(), s, entry));
                }
            }
        } else {
            if term_lower.is_empty() {
                for loc in recent_locations() {
                    let entry = LauncherEntry::new(
                        LauncherIcon::Recent,
                        loc.title,
                        LauncherGroup::Recent,
                        LauncherAction::Navigate(loc.path.clone()),
                    )
                    .meta(loc.path);
                    scored.push((entry.group.order(), 0, entry));
                }
            }

            for section in SECTIONS {
                for item in section.items {
                    let entry = LauncherEntry::new(
                        LauncherIcon::Nav(item.icon),
                        item.label,
                        LauncherGroup::Pages,
                        LauncherAction::Navigate(item.href.to_string()),
                    )
                    .meta(section.label.unwrap_or(""));
                    if let Some(s) = score(&entry.haystack, &term_lower) {
                        scored.push((entry.group.order(), s, entry));
                    }
                }
            }

            if !term_lower.is_empty() {
                let cat = catalog.get().unwrap_or_default();
                let mut seen_docs = HashSet::new();
                for doc in &cat.documents {
                    let entry = document_entry(doc);
                    if let Some(s) = score(&entry.haystack, &term_lower) {
                        if let LauncherAction::Navigate(path) = &entry.action {
                            seen_docs.insert(path.clone());
                        }
                        scored.push((entry.group.order(), s, entry));
                    }
                }
                for doc in docs.get().unwrap_or_default() {
                    let entry = document_entry(&doc);
                    if let LauncherAction::Navigate(path) = &entry.action {
                        if !seen_docs.insert(path.clone()) {
                            continue;
                        }
                    }
                    let s = score(&entry.haystack, &term_lower).unwrap_or(0);
                    scored.push((entry.group.order(), s, entry));
                }
                for dataset in &cat.datasets {
                    let entry = dataset_entry(dataset);
                    if let Some(s) = score(&entry.haystack, &term_lower) {
                        scored.push((entry.group.order(), s, entry));
                    }
                }
                for run in &cat.runs {
                    let entry = run_entry(run);
                    if let Some(s) = score(&entry.haystack, &term_lower) {
                        scored.push((entry.group.order(), s, entry));
                    }
                }
                for connector in &cat.connectors {
                    let entry = connector_entry(connector);
                    if let Some(s) = score(&entry.haystack, &term_lower) {
                        scored.push((entry.group.order(), s, entry));
                    }
                }
            }
        }

        scored.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        scored
            .into_iter()
            .take(MAX_RESULTS)
            .map(|(_, _, entry)| entry)
            .collect::<Vec<_>>()
    });

    let docs_loading = Memo::new(move |_| {
        !command_mode.get() && !nav_term.get().is_empty() && docs.get().is_none()
    });

    let activate = Callback::new(move |idx: usize| {
        let list = results.get_untracked();
        if let Some(entry) = list.get(idx) {
            match &entry.action {
                LauncherAction::Navigate(path) => {
                    use_navigate()(path, NavigateOptions::default());
                }
                LauncherAction::Run(cb) => cb.run(()),
            }
            launcher.close();
        }
    });

    Effect::new(move |_| {
        if open.get() {
            selected.set(0);
            if let Some(el) = input_ref.get() {
                _ = el.focus();
            }
        } else {
            query.set(String::new());
            selected.set(0);
        }
    });

    Effect::new(move |_| {
        let _ = results.get();
        selected.set(0);
    });

    #[cfg(feature = "hydrate")]
    {
        Effect::new(move |prev: Option<()>| {
            let t = nav_term.get();
            if prev.is_none() {
                return;
            }
            let pending = t.clone();
            leptos::task::spawn_local(async move {
                TimeoutFuture::new(DEBOUNCE_MS).await;
                if nav_term.get_untracked() == pending {
                    debounced.set(pending);
                }
            });
        });

        Effect::new(move |_| {
            if !open.get() {
                return;
            }
            let _ = selected.get();
            scroll_active_into_view();
        });

        install_global_shortcut(open, query);
    }

    let on_keydown = move |ev: KeyboardEvent| match ev.key().as_str() {
        "ArrowDown" => {
            ev.prevent_default();
            let len = results.get_untracked().len();
            if len > 0 {
                selected.update(|i| *i = if *i + 1 >= len { 0 } else { *i + 1 });
            }
        }
        "ArrowUp" => {
            ev.prevent_default();
            let len = results.get_untracked().len();
            if len > 0 {
                selected.update(|i| *i = if *i == 0 { len - 1 } else { *i - 1 });
            }
        }
        "Enter" => {
            ev.prevent_default();
            activate.run(selected.get_untracked());
        }
        "Escape" => {
            ev.prevent_default();
            launcher.close();
        }
        _ => {}
    };

    view! {
        <Show when=move || open.get()>
            <div
                class="launcher-backdrop"
                on:mousedown=move |ev| {
                    if ev.target() == ev.current_target() {
                        launcher.close();
                    }
                }
            >
                <div class="launcher-panel" role="dialog" aria-label="Command launcher">
                    <div class="launcher-input-row">
                        <span class="launcher-prompt" aria-hidden="true">
                            {move || if command_mode.get() { "›" } else { "⌕" }}
                        </span>
                        <input
                            node_ref=input_ref
                            type="text"
                            class="launcher-input"
                            placeholder="Search pages & documents… type > for commands"
                            autocomplete="off"
                            spellcheck="false"
                            prop:value=move || query.get()
                            on:input=move |ev| query.set(event_target_value(&ev))
                            on:keydown=on_keydown
                        />
                        {move || docs_loading.get().then(|| view! {
                            <span class="launcher-loading" aria-hidden="true">"searching…"</span>
                        })}
                    </div>

                    <div class="launcher-results" role="listbox">
                        {move || {
                            let list = results.get();
                            let sel = selected.get().min(list.len().saturating_sub(1));
                            if list.is_empty() {
                                return view! {
                                    <div class="launcher-empty">
                                        {move || if term.get().is_empty() {
                                            "Type to search, or > for commands".to_string()
                                        } else {
                                            format!("No matches for \"{}\"", term.get())
                                        }}
                                    </div>
                                }.into_any();
                            }
                            let mut last_group: Option<LauncherGroup> = None;
                            list.into_iter().enumerate().map(|(idx, entry)| {
                                let header = (last_group != Some(entry.group)).then(|| {
                                    view! {
                                        <div class="launcher-group eyebrow">{entry.group.label()}</div>
                                    }
                                });
                                last_group = Some(entry.group);
                                let row_class = if idx == sel {
                                    "launcher-row is-active"
                                } else {
                                    "launcher-row"
                                };
                                let status = entry.status.clone();
                                let keys = entry.keys.clone();
                                let meta = entry.meta.clone();
                                view! {
                                    {header}
                                    <button
                                        type="button"
                                        class=row_class
                                        data-launcher-active=move || if idx == sel { "1" } else { "0" }
                                        role="option"
                                        aria-selected=move || if idx == sel { "true" } else { "false" }
                                        on:mousemove=move |_| selected.set(idx)
                                        on:click=move |_| activate.run(idx)
                                    >
                                        <span class="launcher-row-icon" aria-hidden="true">
                                            {render_icon(entry.icon)}
                                        </span>
                                        <span class="launcher-row-label">{entry.label}</span>
                                        {(!meta.is_empty()).then(|| view! {
                                            <span class="launcher-row-meta">{meta}</span>
                                        })}
                                        {status.map(|(kind, label)| view! {
                                            <StatusPill label=label kind=kind />
                                        })}
                                        {keys.map(|k| view! {
                                            <span class="launcher-row-keys">{k}</span>
                                        })}
                                    </button>
                                }
                            }).collect_view().into_any()
                        }}
                    </div>

                    <div class="launcher-footer">
                        <span><kbd>"↑"</kbd><kbd>"↓"</kbd>" move"</span>
                        <span><kbd>"↵"</kbd>" open"</span>
                        <span><kbd>">"</kbd>" commands"</span>
                        <span><kbd>"esc"</kbd>" close"</span>
                    </div>
                </div>
            </div>
        </Show>
    }
}

fn parse_query(raw: &str) -> (bool, String) {
    if let Some(rest) = raw.strip_prefix('>') {
        (true, rest.trim().to_string())
    } else {
        (false, raw.trim().to_string())
    }
}

#[cfg(feature = "hydrate")]
fn scroll_active_into_view() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Ok(Some(el)) = document.query_selector("[data-launcher-active=\"1\"]") {
        el.scroll_into_view_with_bool(false);
    }
}

#[cfg(feature = "hydrate")]
fn install_global_shortcut(open: RwSignal<bool>, query: RwSignal<String>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let closure =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
            if !(ev.ctrl_key() || ev.meta_key()) || ev.code() != "KeyK" {
                return;
            }
            ev.prevent_default();
            if ev.alt_key() {
                query.set(">".to_string());
                open.set(true);
            } else {
                let next = !open.get_untracked();
                if next {
                    query.set(String::new());
                }
                open.set(next);
            }
        });
    _ = window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
    closure.forget();
}
