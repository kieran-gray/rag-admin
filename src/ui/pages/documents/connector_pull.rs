use std::collections::HashSet;

use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::server_functions::configuration::get_index_profiles;
use crate::server_functions::connector::list_connectors;
use crate::server_functions::connector_sync::{
    bulk_import_from_connector, list_connector_discovered, run_connector_sync,
};
use crate::shared::contracts::{
    aggregate_type, BulkImportResultDto, ConnectorDiscoveredItemViewDto, ConnectorItemStatusDto,
    IndexProfileDto,
};
use crate::ui::components::primitives::{
    ActionItem, ActionsMenu, EmptyState, InlineStatus, InlineStatusMessage, PageHeader, Status,
    StatusPill, Surface, TitleCell,
};
use crate::ui::pages::shared::format_when;
use crate::ui::state::event_bus::use_invalidator;

#[component]
pub fn ConnectorPullPage() -> impl IntoView {
    let params = use_params_map();
    let connector_id = Memo::new(move |_| {
        params
            .with(|p| p.get("connector_id").and_then(|s| Uuid::parse_str(&s).ok()))
            .unwrap_or_default()
    });

    let invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::CONNECTOR,
            aggregate_type::SOURCE_DOCUMENT,
            aggregate_type::INDEXING,
        ])
    });
    let (refresh, set_refresh) = signal(0u32);

    let connector = Resource::new(
        move || (connector_id.get(), invalidator.get()),
        |(id, _)| async move {
            list_connectors()
                .await
                .ok()
                .and_then(|list| list.into_iter().find(|c| c.connector_id == id))
        },
    );

    let index_profiles = Resource::new(
        move || invalidator.get(),
        |_| async move { get_index_profiles().await.unwrap_or_default() },
    );

    let discovered = Resource::new(
        move || (connector_id.get(), refresh.get(), invalidator.get()),
        |(id, _, _)| async move {
            list_connector_discovered(id)
                .await
                .map_err(|e| e.to_string())
        },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);
    let (selected, set_selected) = signal::<HashSet<String>>(HashSet::new());

    let on_sync = move |_| {
        if busy.get_untracked() {
            return;
        }
        let id = connector_id.get_untracked();
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match run_connector_sync(id).await {
                Ok(_) => {
                    set_status.set(Some(InlineStatusMessage::ok("Sync complete")));
                    set_refresh.update(|v| *v += 1);
                }
                Err(e) => {
                    set_status.set(Some(InlineStatusMessage::err(format!("Sync failed: {e}"))));
                }
            }
            set_busy.set(false);
        });
    };

    let import_keys = move |keys: Vec<String>, index_after: bool| {
        if keys.is_empty() {
            set_status.set(Some(InlineStatusMessage::err("Nothing selected")));
            return;
        }
        if busy.get_untracked() {
            return;
        }
        let id = connector_id.get_untracked();
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match bulk_import_from_connector(id, keys, index_after).await {
                Ok(result) => {
                    if result.failures.is_empty() {
                        let msg = if index_after {
                            format!("Added {} · indexing {}", result.imported, result.indexed)
                        } else {
                            format!("Added {} to corpus", result.imported)
                        };
                        set_status.set(Some(InlineStatusMessage::ok(msg)));
                        set_selected.set(HashSet::new());
                    } else {
                        set_status.set(Some(InlineStatusMessage::err(summarize_failures(&result))));
                    }
                    set_refresh.update(|v| *v += 1);
                }
                Err(e) => {
                    set_status.set(Some(InlineStatusMessage::err(format!("Add failed: {e}"))));
                }
            }
            set_busy.set(false);
        });
    };

    let new_keys = move || -> Vec<String> {
        discovered
            .get()
            .and_then(Result::ok)
            .map(|list| {
                list.into_iter()
                    .filter(|i| matches!(i.status, ConnectorItemStatusDto::Discovered))
                    .map(|i| i.source_ref_key)
                    .collect()
            })
            .unwrap_or_default()
    };

    let import_selected = move |index_after: bool| {
        import_keys(selected.get_untracked().into_iter().collect(), index_after);
    };
    let add_all_new = move |_| {
        let keys = new_keys();
        if keys.is_empty() {
            set_status.set(Some(InlineStatusMessage::err("Nothing new to add")));
            return;
        }
        import_keys(keys, true);
    };

    let nothing_selected = Signal::derive(move || selected.with(HashSet::is_empty));
    let busy_or_no_selection = Signal::derive(move || busy.get() || nothing_selected.get());
    let busy_signal = Signal::derive(move || busy.get());

    let more_actions = move || {
        vec![
            ActionItem::new(
                "Add selected (no index)",
                Callback::new(move |_| import_selected(false)),
            )
            .disabled(busy_or_no_selection),
            ActionItem::new("Add all new (+ index)", Callback::new(add_all_new))
                .disabled(busy_signal),
        ]
    };

    view! {
        <div>
            {move || {
                let name = connector
                    .get()
                    .flatten()
                    .map(|c| c.name)
                    .unwrap_or_else(|| "Connector".into());
                view! {
                    <PageHeader
                        title=name
                        subtitle="Sync, browse discovered items, and pull them into your corpus.".to_string()
                        actions=Box::new(move || view! {
                            <button type="button" class="btn" disabled=busy on:click=on_sync>
                                {move || if busy.get() { "Syncing…" } else { "Sync now" }}
                            </button>
                        }.into_any())
                    />
                }
            }}

            <div class="flex flex-col gap-3">
                <Transition fallback=|| ()>
                    {move || {
                        let profiles = index_profiles.get().unwrap_or_default();
                        let explicit = connector.get().flatten().and_then(|c| c.default_index_profile_id);
                        view! { <TargetIndicator profiles=profiles explicit=explicit /> }
                    }}
                </Transition>

                <InlineStatus status=status />

                <Surface flush=true>
                    <div class="list-page-toolbar">
                        {move || nothing_selected.get().then(|| view! {
                            <span class="text-xs muted">"Select items to add to your corpus."</span>
                        })}
                        <div class="list-page-pagination-spacer"></div>
                        <ActionsMenu label="More".to_string() items=more_actions() />
                        <button
                            type="button"
                            class="btn btn-primary"
                            disabled=busy_or_no_selection
                            on:click=move |_| import_selected(true)
                        >
                            "Add & index selected"
                        </button>
                    </div>

                    <Transition fallback=|| view! { <div class="p-6 muted text-sm">"Loading…"</div> }>
                        {move || discovered.get().map(|res| match res {
                            Err(e) => Either::Left(view! {
                                <div class="p-6 log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                            }),
                            Ok(list) if list.is_empty() => Either::Right(Either::Left(view! {
                                <div class="p-6">
                                    <EmptyState
                                        title="Nothing discovered yet"
                                        body="Run a sync to discover what this connector can pull in.".to_string()
                                    />
                                </div>
                            })),
                            Ok(list) => Either::Right(Either::Right(view! {
                                <DiscoveredTable items=list selected=selected set_selected=set_selected />
                            })),
                        })}
                    </Transition>
                </Surface>
            </div>
        </div>
    }
}

#[allow(clippy::needless_pass_by_value)]
#[component]
fn TargetIndicator(profiles: Vec<IndexProfileDto>, explicit: Option<Uuid>) -> impl IntoView {
    match resolve_target(&profiles, explicit) {
        None => view! {
            <div class="text-xs muted flex items-center gap-2">
                "No index profile configured — "
                <A href="/pipeline/profiles">"set a default →"</A>
            </div>
        }
        .into_any(),
        Some((profile, is_explicit)) => {
            let source = if is_explicit {
                "connector"
            } else {
                "app default"
            };
            let dims = format!("{}d", profile.dimensions);
            view! {
                <div class="flex items-center gap-2 text-xs flex-wrap">
                    <span class="eyebrow">"Indexing into"</span>
                    <span class="text-sm text-text">{profile.name}</span>
                    <span class="pill pill-neutral text-xs">{dims}</span>
                    <span class="pill pill-neutral text-xs">{source}</span>
                </div>
            }
            .into_any()
        }
    }
}

fn resolve_target(
    profiles: &[IndexProfileDto],
    explicit: Option<Uuid>,
) -> Option<(IndexProfileDto, bool)> {
    if let Some(id) = explicit {
        if let Some(p) = profiles.iter().find(|p| p.index_profile_id == id) {
            return Some((p.clone(), true));
        }
    }
    profiles
        .iter()
        .find(|p| p.is_default)
        .cloned()
        .map(|p| (p, false))
}

fn summarize_failures(result: &BulkImportResultDto) -> String {
    let first = result
        .failures
        .first()
        .map(|f| f.error.clone())
        .unwrap_or_default();
    let more = result.failures.len().saturating_sub(1);
    let suffix = if more > 0 {
        format!(" (+{more} more)")
    } else {
        String::new()
    };
    format!(
        "Added {} to corpus, {} failed: {first}{suffix}",
        result.imported,
        result.failures.len()
    )
}

#[component]
fn DiscoveredTable(
    items: Vec<ConnectorDiscoveredItemViewDto>,
    selected: ReadSignal<HashSet<String>>,
    set_selected: WriteSignal<HashSet<String>>,
) -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th class="w-8"></th>
                    <th class="w-[52%]">"Title"</th>
                    <th>"Status"</th>
                    <th>"Last seen"</th>
                </tr>
            </thead>
            <tbody>
                {items.into_iter().map(|item| {
                    let key_check = item.source_ref_key.clone();
                    let key_toggle = item.source_ref_key.clone();
                    let title = item.title.clone();
                    let sub = item.source_ref_key.clone();
                    let (label, kind) = status_for(item.status);
                    let last_seen = format_when(&item.last_seen);
                    let last_seen_full = item.last_seen.clone();
                    view! {
                        <tr>
                            <td>
                                <input
                                    type="checkbox"
                                    prop:checked=move || selected.with(|s| s.contains(&key_check))
                                    on:change=move |_| {
                                        let k = key_toggle.clone();
                                        set_selected.update(|s| {
                                            if !s.remove(&k) {
                                                s.insert(k);
                                            }
                                        });
                                    }
                                />
                            </td>
                            <td><TitleCell title=title sub=sub /></td>
                            <td><StatusPill label=label.to_string() kind=kind /></td>
                            <td class="text-xs muted" title=last_seen_full>{last_seen}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }
}

fn status_for(status: ConnectorItemStatusDto) -> (&'static str, Status) {
    match status {
        ConnectorItemStatusDto::Discovered => ("Available", Status::Stale),
        ConnectorItemStatusDto::Imported => ("In corpus", Status::Info),
        ConnectorItemStatusDto::Indexed => ("Indexed", Status::Ok),
    }
}
