use std::collections::HashMap;

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use uuid::Uuid;

use crate::server_functions::connector::list_connectors;
use crate::shared::contracts::{
    aggregate_type, ConnectorDto, DocumentListPageDto, DocumentStatusFilterDto, SourceFacetDto,
};
use crate::ui::state::event_bus::use_invalidator;

const STATUS_KEY: &str = "status";
const SOURCE_KEY: &str = "source";
const CONNECTOR_KEY: &str = "connector";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipsState {
    pub statuses: Vec<DocumentStatusFilterDto>,
    pub sources: Vec<String>,
    pub connectors: Vec<Uuid>,
}

impl ChipsState {
    pub fn is_empty(&self) -> bool {
        self.statuses.is_empty() && self.sources.is_empty() && self.connectors.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutocompleteKind {
    Status,
    Source,
    Connector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FilterKeyDef {
    key: &'static str,
    hint: &'static str,
}

const FILTER_KEYS: &[FilterKeyDef] = &[
    FilterKeyDef {
        key: STATUS_KEY,
        hint: "indexed, failed, in progress…",
    },
    FilterKeyDef {
        key: SOURCE_KEY,
        hint: "host or upload",
    },
    FilterKeyDef {
        key: CONNECTOR_KEY,
        hint: "connector that ingested it",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Panel {
    Keys(Vec<FilterKeyDef>),
    Values(AutocompleteKind, Vec<Suggestion>),
}

impl Panel {
    fn len(&self) -> usize {
        match self {
            Panel::Keys(keys) => keys.len(),
            Panel::Values(_, items) => items.len(),
        }
    }
}

fn matching_keys(draft: &str) -> Vec<FilterKeyDef> {
    let needle = draft.trim().to_lowercase();
    if needle.is_empty() || needle.contains(':') {
        return Vec::new();
    }
    FILTER_KEYS
        .iter()
        .copied()
        .filter(|def| def.key.starts_with(&needle))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Suggestion {
    Status(DocumentStatusFilterDto),
    Source(String),
    Connector(Uuid, String),
}

impl Suggestion {
    fn label(&self) -> String {
        match self {
            Suggestion::Status(s) => s.label().to_string(),
            Suggestion::Source(host) => host.clone(),
            Suggestion::Connector(_, name) => name.clone(),
        }
    }

    fn token_value(&self) -> String {
        match self {
            Suggestion::Status(s) => s.slug().to_string(),
            Suggestion::Source(host) => host.clone(),
            Suggestion::Connector(id, _) => id.to_string(),
        }
    }
}

fn classify_draft(draft: &str) -> Option<(AutocompleteKind, &str)> {
    let lower = draft.trim_start();
    if let Some(rest) = lower.strip_prefix(&format!("{STATUS_KEY}:")) {
        return Some((AutocompleteKind::Status, rest));
    }
    if let Some(rest) = lower.strip_prefix(&format!("{SOURCE_KEY}:")) {
        return Some((AutocompleteKind::Source, rest));
    }
    if let Some(rest) = lower.strip_prefix(&format!("{CONNECTOR_KEY}:")) {
        return Some((AutocompleteKind::Connector, rest));
    }
    None
}

pub struct SmartFilterCallbacks {
    pub set_search: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
    pub set_statuses: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
    pub set_sources: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
    pub set_connectors: Box<dyn Fn(Option<String>) + Send + Sync + 'static>,
}

#[component]
pub fn SmartFilterBar(
    search: Signal<String>,
    chips: Signal<ChipsState>,
    page: Resource<Result<DocumentListPageDto, ServerFnError>>,
    total_matching: Signal<Option<u64>>,
    on_search_change: Callback<Option<String>>,
    on_toggle_status: Callback<DocumentStatusFilterDto>,
    on_toggle_source: Callback<String>,
    on_toggle_connector: Callback<Uuid>,
    on_clear: Callback<()>,
) -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::CONNECTOR]));
    let connectors_resource = Resource::new(
        move || invalidator.get(),
        |_| async move { list_connectors().await.map_err(|e| e.to_string()) },
    );

    let (draft, set_draft) = signal::<String>(search.get_untracked());
    let (highlighted, set_highlighted) = signal::<usize>(0);

    Effect::new(move |_| {
        let s = search.get();
        let current = draft.get_untracked();
        if classify_draft(&current).is_some() {
            return;
        }
        if current != s {
            set_draft.set(s);
        }
    });

    let status_options: Vec<DocumentStatusFilterDto> = vec![
        DocumentStatusFilterDto::Indexed,
        DocumentStatusFilterDto::InProgress,
        DocumentStatusFilterDto::Failed,
        DocumentStatusFilterDto::Registered,
    ];

    let status_options_for_match = status_options.clone();

    let panel = Memo::new(move |_| {
        let draft_value = draft.get();
        let Some((kind, rest)) = classify_draft(&draft_value) else {
            let keys = matching_keys(&draft_value);
            return (!keys.is_empty()).then_some(Panel::Keys(keys));
        };
        let needle = rest.to_lowercase();
        let chips_now = chips.get();

        let items: Vec<Suggestion> = match kind {
            AutocompleteKind::Status => status_options_for_match
                .iter()
                .copied()
                .filter(|s| !chips_now.statuses.contains(s))
                .filter(|s| {
                    needle.is_empty()
                        || s.slug().contains(&needle)
                        || s.label().to_lowercase().contains(&needle)
                })
                .map(Suggestion::Status)
                .collect(),
            AutocompleteKind::Source => {
                let host_facets: Vec<SourceFacetDto> = page
                    .get()
                    .and_then(Result::ok)
                    .map(|p| p.sources.clone())
                    .unwrap_or_default();
                host_facets
                    .into_iter()
                    .map(|f| f.source.filter_key())
                    .filter(|k| !chips_now.sources.contains(k))
                    .filter(|k| needle.is_empty() || k.to_lowercase().contains(&needle))
                    .map(Suggestion::Source)
                    .collect()
            }
            AutocompleteKind::Connector => {
                let list: Vec<ConnectorDto> = connectors_resource
                    .get()
                    .and_then(Result::ok)
                    .unwrap_or_default();
                list.into_iter()
                    .filter(|c| !chips_now.connectors.contains(&c.connector_id))
                    .filter(|c| {
                        needle.is_empty()
                            || c.name.to_lowercase().contains(&needle)
                            || c.connector_id.to_string().contains(&needle)
                    })
                    .map(|c| Suggestion::Connector(c.connector_id, c.name))
                    .collect()
            }
        };

        Some(Panel::Values(kind, items))
    });

    Effect::new(move |_| {
        let _ = panel.get();
        set_highlighted.set(0);
    });

    let chips_now_for_input = chips;
    let on_input = move |ev: leptos::ev::Event| {
        let value = event_target_value(&ev);
        set_draft.set(value.clone());
        if classify_draft(&value).is_none() {
            on_search_change.run(if value.trim().is_empty() {
                None
            } else {
                Some(value)
            });
        } else {
            on_search_change.run(None);
        }
    };

    let commit_suggestion = move |s: Suggestion| {
        match s {
            Suggestion::Status(st) => on_toggle_status.run(st),
            Suggestion::Source(host) => on_toggle_source.run(host),
            Suggestion::Connector(id, _) => on_toggle_connector.run(id),
        }
        set_draft.set(String::new());
        on_search_change.run(None);
        set_highlighted.set(0);
    };

    let complete_key = move |key: &str| {
        set_draft.set(format!("{key}:"));
        on_search_change.run(None);
        set_highlighted.set(0);
    };

    let remove_last_chip = move || {
        let c = chips_now_for_input.get_untracked();
        if let Some(last_connector) = c.connectors.last().copied() {
            on_toggle_connector.run(last_connector);
        } else if let Some(last_source) = c.sources.last().cloned() {
            on_toggle_source.run(last_source);
        } else if let Some(last_status) = c.statuses.last().copied() {
            on_toggle_status.run(last_status);
        }
    };

    let on_keydown = move |ev: KeyboardEvent| {
        let key = ev.key();
        let key_str = key.as_str();
        let current_panel = panel.get();
        let len = current_panel.as_ref().map_or(0, Panel::len);

        let commit_highlighted = |ev: &KeyboardEvent| {
            let Some(panel) = current_panel.as_ref() else {
                return;
            };
            let idx = highlighted.get_untracked().min(len.saturating_sub(1));
            ev.prevent_default();
            match panel {
                Panel::Keys(keys) => {
                    if let Some(def) = keys.get(idx) {
                        complete_key(def.key);
                    }
                }
                Panel::Values(_, items) => {
                    if let Some(s) = items.get(idx).cloned() {
                        commit_suggestion(s);
                    }
                }
            }
        };

        match key_str {
            "Backspace" if draft.get_untracked().is_empty() => {
                ev.prevent_default();
                remove_last_chip();
            }
            "Enter" | "Tab" if len > 0 => {
                commit_highlighted(&ev);
            }
            "Escape" => {
                set_draft.set(String::new());
                on_search_change.run(None);
            }
            "ArrowDown" if len > 0 => {
                ev.prevent_default();
                let max = len.saturating_sub(1);
                set_highlighted.update(|i| {
                    *i = if *i >= max { 0 } else { *i + 1 };
                });
            }
            "ArrowUp" if len > 0 => {
                ev.prevent_default();
                let max = len.saturating_sub(1);
                set_highlighted.update(|i| {
                    *i = if *i == 0 { max } else { *i - 1 };
                });
            }
            _ => {}
        }
    };

    let has_filters =
        Memo::new(move |_| !chips.get().is_empty() || !search.get().trim().is_empty());

    view! {
        <div class="smart-filter">
            <div class="smart-filter-shell">
                <span class="smart-filter-icon" aria-hidden="true">"⌕"</span>

                <ChipStrip
                    chips=chips
                    page=page
                    connectors=connectors_resource
                    on_toggle_status=on_toggle_status
                    on_toggle_source=on_toggle_source
                    on_toggle_connector=on_toggle_connector
                />

                <input
                    type="text"
                    class="smart-filter-input"
                    placeholder=move || {
                        if chips.get().is_empty() && search.get().is_empty() {
                            "Search titles or URLs… try status:, source:, connector:".to_string()
                        } else {
                            String::new()
                        }
                    }
                    prop:value=move || draft.get()
                    on:input=on_input
                    on:keydown=on_keydown
                    autocomplete="off"
                    spellcheck="false"
                />

                {move || (classify_draft(&draft.get()).is_none() && draft.get().is_empty()).then(|| view! {
                    <span class="smart-filter-hint" aria-hidden="true">"↵ search"</span>
                })}

                {move || has_filters.get().then(|| view! {
                    <button
                        type="button"
                        class="smart-filter-clear"
                        on:click=move |_| on_clear.run(())
                    >
                        "Clear"
                    </button>
                })}
            </div>

            {move || panel.get().map(|p| match p {
                Panel::Keys(keys) => view! {
                    <KeyPopover
                        keys=keys
                        highlighted=highlighted
                        on_pick=Callback::new(move |def: FilterKeyDef| complete_key(def.key))
                    />
                }.into_any(),
                Panel::Values(kind, items) => view! {
                    <AutocompletePopover
                        kind=kind
                        items=items
                        highlighted=highlighted
                        on_pick=Callback::new(move |s: Suggestion| commit_suggestion(s))
                    />
                }.into_any(),
            })}
        </div>

        <SummaryLine
            total_matching=total_matching
            page=page
            on_clear=on_clear
            has_filters=has_filters
        />
    }
}

#[component]
fn ChipStrip(
    chips: Signal<ChipsState>,
    page: Resource<Result<DocumentListPageDto, ServerFnError>>,
    connectors: Resource<Result<Vec<ConnectorDto>, String>>,
    on_toggle_status: Callback<DocumentStatusFilterDto>,
    on_toggle_source: Callback<String>,
    on_toggle_connector: Callback<Uuid>,
) -> impl IntoView {
    let source_labels = Memo::new(move |_| {
        page.get()
            .and_then(Result::ok)
            .map(|p| {
                p.sources
                    .iter()
                    .map(|f| (f.source.filter_key(), f.source.label()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default()
    });
    let connector_labels = Memo::new(move |_| {
        connectors
            .get()
            .and_then(Result::ok)
            .map(|list| {
                list.into_iter()
                    .map(|c| (c.connector_id, c.name))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default()
    });

    view! {
        {move || {
            let c = chips.get();
            view! {
                <>
                    {c.statuses.into_iter().map(|s| view! {
                        <SmartChip
                            key_label="status".to_string()
                            value_label=s.label().to_string()
                            on_remove=Callback::new(move |_| on_toggle_status.run(s))
                        />
                    }).collect_view()}
                    {c.sources.into_iter().map(|key| {
                        let key_for_callback = key.clone();
                        let label = source_labels.get().get(&key).cloned().unwrap_or(key);
                        view! {
                            <SmartChip
                                key_label="source".to_string()
                                value_label=label
                                on_remove=Callback::new(move |_| on_toggle_source.run(key_for_callback.clone()))
                            />
                        }
                    }).collect_view()}
                    {c.connectors.into_iter().map(|id| {
                        let label = connector_labels.get().get(&id).cloned().unwrap_or_else(|| id.to_string());
                        view! {
                            <SmartChip
                                key_label="connector".to_string()
                                value_label=label
                                on_remove=Callback::new(move |_| on_toggle_connector.run(id))
                            />
                        }
                    }).collect_view()}
                </>
            }
        }}
    }
}

#[component]
fn SmartChip(key_label: String, value_label: String, on_remove: Callback<()>) -> impl IntoView {
    view! {
        <span class="smart-filter-chip">
            <span class="smart-filter-chip-key">{format!("{key_label}:")}</span>
            <span>{value_label}</span>
            <button
                type="button"
                class="smart-filter-chip-remove"
                aria-label="Remove filter"
                on:click=move |_| on_remove.run(())
            >
                "×"
            </button>
        </span>
    }
}

#[component]
fn KeyPopover(
    keys: Vec<FilterKeyDef>,
    highlighted: ReadSignal<usize>,
    on_pick: Callback<FilterKeyDef>,
) -> impl IntoView {
    view! {
        <div class="smart-filter-popover" role="listbox">
            <div class="smart-filter-popover-header">"Add filter"</div>
            {keys.into_iter().enumerate().map(|(idx, def)| {
                view! {
                    <div
                        class="smart-filter-popover-item"
                        role="option"
                        data-highlighted=move || (highlighted.get() == idx).to_string()
                        on:mousedown=move |ev| {
                            ev.prevent_default();
                            on_pick.run(def);
                        }
                    >
                        <span class="smart-filter-popover-item-label">{format!("{}:", def.key)}</span>
                        <span class="smart-filter-popover-item-hint">{def.hint}</span>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn AutocompletePopover(
    kind: AutocompleteKind,
    items: Vec<Suggestion>,
    highlighted: ReadSignal<usize>,
    on_pick: Callback<Suggestion>,
) -> impl IntoView {
    let header = match kind {
        AutocompleteKind::Status => "Status",
        AutocompleteKind::Source => "Source",
        AutocompleteKind::Connector => "Connector",
    };

    view! {
        <div class="smart-filter-popover" role="listbox">
            <div class="smart-filter-popover-header">{header}</div>
            {if items.is_empty() {
                view! { <div class="smart-filter-popover-empty">"No matches"</div> }.into_any()
            } else {
                view! {
                    <>
                        {items.into_iter().enumerate().map(|(idx, suggestion)| {
                            let label = suggestion.label();
                            let token_value = suggestion.token_value();
                            let suggestion_for_click = suggestion.clone();
                            view! {
                                <div
                                    class="smart-filter-popover-item"
                                    role="option"
                                    data-highlighted=move || (highlighted.get() == idx).to_string()
                                    on:mousedown=move |ev| {
                                        ev.prevent_default();
                                        on_pick.run(suggestion_for_click.clone());
                                    }
                                >
                                    <span class="smart-filter-popover-item-label">{label}</span>
                                    <span class="smart-filter-popover-item-kbd">{token_value}</span>
                                </div>
                            }
                        }).collect_view()}
                    </>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn SummaryLine(
    total_matching: Signal<Option<u64>>,
    page: Resource<Result<DocumentListPageDto, ServerFnError>>,
    on_clear: Callback<()>,
    has_filters: Memo<bool>,
) -> impl IntoView {
    view! {
        <div class="smart-filter-summary">
            <Transition fallback=|| view! { <span class="faint">"…"</span> }>
                {move || {
                    let total = total_matching.get();
                    let all = page.get().and_then(Result::ok).map(|p| p.total_all);
                    match (total, all) {
                        (Some(matching), Some(all_total)) if matching != all_total => {
                            format!("{matching} of {all_total} match")
                        }
                        (Some(matching), _) => {
                            format!("{matching} match{}", if matching == 1 { "" } else { "es" })
                        }
                        _ => "—".to_string(),
                    }
                }}
            </Transition>
            {move || has_filters.get().then(|| view! {
                <button
                    type="button"
                    class="smart-filter-clear"
                    on:click=move |_| on_clear.run(())
                >
                    "Clear all"
                </button>
            })}
        </div>
    }
}
