use std::collections::HashSet;

use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::configuration::{get_chunking_configurations, get_index_profiles};
use crate::server_functions::connector::list_connectors;
use crate::server_functions::connector_sync::{
    bulk_import_from_connector, list_connector_discovered, list_connector_syncs, run_connector_sync,
};
use crate::shared::contracts::{
    aggregate_type, ChunkingConfigurationDto, ConnectorDiscoveredItemViewDto, ConnectorDto,
    ConnectorItemStatusDto, ConnectorSyncSummaryDto, IndexProfileDto,
};
use crate::ui::components::primitives::{
    ActionItem, ActionsMenu, Dialog, Help, InlineStatus, InlineStatusMessage, Status, StatusPill,
    Surface, TitleCell,
};
use crate::ui::pages::shared::format_when;
use crate::ui::state::event_bus::use_invalidator;

#[component]
pub fn ConnectorBrowseView(connector_id: Signal<Uuid>) -> impl IntoView {
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
                .map(|list| list.into_iter().find(|c| c.connector_id == id))
                .map_err(|e| e.to_string())
        },
    );

    let latest_sync = Resource::new(
        move || (connector_id.get(), refresh.get(), invalidator.get()),
        |(id, _, _)| async move {
            list_connector_syncs(id, 1, 0)
                .await
                .map(|mut list| list.pop())
                .map_err(|e| e.to_string())
        },
    );

    let discovered = Resource::new(
        move || (connector_id.get(), refresh.get(), invalidator.get()),
        |(id, _, _)| async move {
            list_connector_discovered(id)
                .await
                .map_err(|e| e.to_string())
        },
    );

    let index_profiles = Resource::new(
        move || invalidator.get(),
        |_| async move { get_index_profiles().await.unwrap_or_default() },
    );

    let chunking_configs = Resource::new(
        move || invalidator.get(),
        |_| async move { get_chunking_configurations().await.unwrap_or_default() },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);
    let (selected, set_selected) = signal::<HashSet<String>>(HashSet::new());

    let on_sync_now = move |_| {
        let id = connector_id.get_untracked();
        if busy.get_untracked() {
            return;
        }
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

    let import_selected = move |index_after: bool| {
        let id = connector_id.get_untracked();
        let keys: Vec<String> = selected.get_untracked().into_iter().collect();
        if keys.is_empty() {
            set_status.set(Some(InlineStatusMessage::err("Nothing selected")));
            return;
        }
        if busy.get_untracked() {
            return;
        }
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match bulk_import_from_connector(id, keys, index_after).await {
                Ok(result) => {
                    let msg = if result.failures.is_empty() {
                        format!(
                            "Imported {}{}",
                            result.imported,
                            if index_after {
                                format!(" (indexed {})", result.indexed)
                            } else {
                                String::new()
                            }
                        )
                    } else {
                        format!(
                            "Imported {}, {} failure(s)",
                            result.imported,
                            result.failures.len()
                        )
                    };
                    let outcome = if result.failures.is_empty() {
                        InlineStatusMessage::ok(msg)
                    } else {
                        InlineStatusMessage::err(msg)
                    };
                    set_status.set(Some(outcome));
                    set_selected.set(HashSet::new());
                    set_refresh.update(|v| *v += 1);
                }
                Err(e) => set_status.set(Some(InlineStatusMessage::err(format!(
                    "Bulk import failed: {e}"
                )))),
            }
            set_busy.set(false);
        });
    };

    let import_all_new = move |_| {
        let all_new: HashSet<String> = discovered
            .get()
            .and_then(Result::ok)
            .map(|list| {
                list.into_iter()
                    .filter(|i| matches!(i.status, ConnectorItemStatusDto::Discovered))
                    .map(|i| i.source_ref_key)
                    .collect()
            })
            .unwrap_or_default();
        if all_new.is_empty() {
            set_status.set(Some(InlineStatusMessage::err("No new items to import")));
            return;
        }
        set_selected.set(all_new);
        import_selected(true);
    };

    view! {
        <div>
            <Suspense fallback=|| view! { <Surface><div class="p-6 muted text-sm">"Loading connector…"</div></Surface> }>
                {move || connector.get().map(|res| match res {
                    Err(e) => Either::Left(view! {
                        <Surface><div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div></Surface>
                    }),
                    Ok(None) => Either::Right(Either::Left(view! {
                        <Surface><div class="muted text-sm p-6">"Connector not found"</div></Surface>
                    })),
                    Ok(Some(c)) => Either::Right(Either::Right(view! {
                        <ConnectorBrowseBody
                            connector=c
                            connector_id=connector_id
                            latest_sync=latest_sync
                            discovered=discovered
                            index_profiles=index_profiles
                            chunking_configs=chunking_configs
                            busy=busy
                            status=status
                            selected=selected
                            set_selected=set_selected
                            on_sync_now=Callback::new(on_sync_now)
                            on_import_selected=Callback::new(import_selected)
                            on_import_all_new=Callback::new(import_all_new)
                        />
                    })),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn ConnectorBrowseBody(
    connector: ConnectorDto,
    connector_id: Signal<Uuid>,
    latest_sync: Resource<Result<Option<ConnectorSyncSummaryDto>, String>>,
    discovered: Resource<Result<Vec<ConnectorDiscoveredItemViewDto>, String>>,
    index_profiles: Resource<Vec<IndexProfileDto>>,
    chunking_configs: Resource<Vec<ChunkingConfigurationDto>>,
    busy: ReadSignal<bool>,
    status: ReadSignal<Option<InlineStatusMessage>>,
    selected: ReadSignal<HashSet<String>>,
    set_selected: WriteSignal<HashSet<String>>,
    on_sync_now: Callback<()>,
    on_import_selected: Callback<bool>,
    on_import_all_new: Callback<()>,
) -> impl IntoView {
    let default_index_profile_id = connector.default_index_profile_id;
    let default_chunking_id = connector.default_chunking_configuration_id;

    let nothing_selected = Signal::derive(move || selected.with(HashSet::is_empty));
    let busy_or_no_selection =
        Signal::derive(move || busy.get() || selected.with(HashSet::is_empty));
    let busy_signal = Signal::derive(move || busy.get());

    let discovered_actions = vec![
        ActionItem::new(
            "Import + index selected",
            Callback::new(move |_| on_import_selected.run(true)),
        )
        .primary()
        .disabled(busy_or_no_selection),
        ActionItem::new(
            "Import selected (no index)",
            Callback::new(move |_| on_import_selected.run(false)),
        )
        .disabled(busy_or_no_selection),
        ActionItem::new(
            "Import all new (+ index)",
            Callback::new(move |_| on_import_all_new.run(())),
        )
        .disabled(busy_signal),
    ];

    let (history_open, set_history_open) = signal(false);

    view! {
        <div class="flex flex-col gap-3">
            <div class="text-sm muted">
                "Bulk import items discovered from this connector. Defaults to the connector's index profile and chunking when indexing."
            </div>

            <InlineStatus status=status />

            <Surface>
                <div class="p-4 flex flex-col gap-3">
                    <div class="text-xs uppercase tracking-wide muted">"Ingestion defaults"</div>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                        <IndexProfileCard
                            default_id=default_index_profile_id
                            profiles=index_profiles
                        />
                        <ChunkingConfigCard
                            default_id=default_chunking_id
                            configs=chunking_configs
                        />
                    </div>
                </div>
            </Surface>

            <Surface>
                <div class="p-4 flex flex-col gap-3">
                    <div class="flex items-center justify-between flex-wrap gap-2">
                        <h3 class="section-title">"Most recent sync"</h3>
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="btn"
                                on:click=move |_| set_history_open.set(true)
                            >
                                "View history"
                            </button>
                            <button
                                type="button"
                                class="btn btn-primary"
                                disabled=busy
                                on:click=move |_| on_sync_now.run(())
                            >
                                {move || if busy.get() { "Syncing…" } else { "Sync now" }}
                            </button>
                        </div>
                    </div>
                    <Suspense fallback=|| view! { <div class="muted text-sm p-2">"Loading…"</div> }>
                        {move || latest_sync.get().map(|res| match res {
                            Err(e) => view! { <div class="log-line-error text-sm">{format!("Failed: {e}")}</div> }.into_any(),
                            Ok(None) => view! { <div class="muted text-sm">"No syncs yet"</div> }.into_any(),
                            Ok(Some(sync)) => view! { <SyncRow sync=sync /> }.into_any(),
                        })}
                    </Suspense>
                </div>
            </Surface>

            <SyncHistoryDialog
                connector_id=connector_id
                open=history_open
                set_open=set_history_open
            />

            <Surface>
                <div class="p-4 flex flex-col gap-3">
                    <div class="flex items-center justify-between flex-wrap gap-2">
                        <div class="flex items-center gap-1">
                            <h3 class="section-title">"Discovered items"</h3>
                            <Help title="Status meanings".to_string() label="What do these statuses mean?".to_string()>
                                <StatusLegend />
                            </Help>
                        </div>
                        <div class="flex items-center gap-2">
                            {move || nothing_selected.get().then(|| view! {
                                <span class="text-xs muted">"Select items to import"</span>
                            })}
                            <ActionsMenu label="Import actions".to_string() items=discovered_actions.clone() />
                        </div>
                    </div>

                    <Suspense fallback=|| view! { <div class="muted text-sm">"Loading…"</div> }>
                        {move || discovered.get().map(|res| match res {
                            Err(e) => view! { <div class="log-line-error text-sm">{format!("Failed: {e}")}</div> }.into_any(),
                            Ok(list) if list.is_empty() => view! { <div class="muted text-sm">"No items discovered yet. Click Sync now to populate."</div> }.into_any(),
                            Ok(list) => view! {
                                <DiscoveredTable
                                    items=list
                                    selected=selected
                                    set_selected=set_selected
                                />
                            }.into_any(),
                        })}
                    </Suspense>
                </div>
            </Surface>
        </div>
    }
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
                    <th class="w-[42%]">"Title"</th>
                    <th>"Status"</th>
                    <th>"Last seen"</th>
                </tr>
            </thead>
            <tbody>
                {items.into_iter().map(|item| {
                    let key_for_check = item.source_ref_key.clone();
                    let key_for_toggle = item.source_ref_key.clone();
                    let title = item.title.clone();
                    let sub_text = item.source_ref_key.clone();
                    let (status_label, status_kind) = match item.status {
                        ConnectorItemStatusDto::Discovered => ("Discovered", Status::Stale),
                        ConnectorItemStatusDto::Imported => ("Imported", Status::Info),
                        ConnectorItemStatusDto::Indexed => ("Indexed", Status::Ok),
                    };
                    let last_seen = format_when(&item.last_seen);
                    let last_seen_full = item.last_seen.clone();
                    view! {
                        <tr>
                            <td>
                                <input
                                    type="checkbox"
                                    prop:checked=move || selected.with(|s| s.contains(&key_for_check))
                                    on:change=move |_| {
                                        let k = key_for_toggle.clone();
                                        set_selected.update(|s| {
                                            if s.contains(&k) {
                                                s.remove(&k);
                                            } else {
                                                s.insert(k);
                                            }
                                        });
                                    }
                                />
                            </td>
                            <td>
                                <TitleCell title=title sub=sub_text />
                            </td>
                            <td><StatusPill label=status_label.to_string() kind=status_kind /></td>
                            <td class="text-xs muted" title=last_seen_full>{last_seen}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn StatusLegend() -> impl IntoView {
    view! {
        <div class="flex flex-col gap-3 text-sm">
            <p class="muted">
                "Discovered items move through three states as you ingest them."
            </p>
            <div class="flex items-start gap-3">
                <StatusPill label="Discovered".to_string() kind=Status::Stale />
                <span>"Surfaced by sync, not stored locally yet. Nothing to query."</span>
            </div>
            <div class="flex items-start gap-3">
                <StatusPill label="Imported".to_string() kind=Status::Info />
                <span>"Markdown is stored locally, but it isn't in a vector index yet — it won't appear in retrieval."</span>
            </div>
            <div class="flex items-start gap-3">
                <StatusPill label="Indexed".to_string() kind=Status::Ok />
                <span>"Chunked, embedded, and live in a vector index. Queryable via retrieval and chat."</span>
            </div>
            <p class="muted text-xs">
                "Use "<strong>"Import selected"</strong>" to just store the content. Use "<strong>"Import + index selected"</strong>" to also chunk + embed into this connector's configured index profile."
            </p>
        </div>
    }
}

#[component]
fn IndexProfileCard(
    default_id: Option<Uuid>,
    profiles: Resource<Vec<IndexProfileDto>>,
) -> impl IntoView {
    view! {
        <div class="surface-raised rounded p-3">
            <div class="text-xs uppercase tracking-wide muted mb-2">"Index profile"</div>
            <Suspense fallback=|| view! { <div class="muted text-sm">"Loading…"</div> }>
                {move || profiles.get().map(|list| {
                    let resolved = resolve_index_profile(&list, default_id);
                    match resolved {
                        None => view! {
                            <div class="muted text-sm">"No index profile configured"</div>
                        }.into_any(),
                        Some((profile, is_explicit)) => {
                            let embedding = profile.embedding_model_name.clone().unwrap_or_else(|| short_uuid(profile.embedding_model_id));
                            let index = profile.vector_index_name.clone().unwrap_or_else(|| short_uuid(profile.vector_index_id));
                            let source_pill = if is_explicit { "connector" } else { "app default" };
                            view! {
                                <div class="flex flex-col gap-2">
                                    <div class="flex items-center gap-2 min-w-0">
                                        <span class="font-medium truncate">{profile.name.clone()}</span>
                                        <span class="pill pill-neutral text-xs">{source_pill}</span>
                                    </div>
                                    <div class="flex flex-wrap gap-1.5 text-xs muted">
                                        <span class="pill pill-neutral">{format!("embed · {embedding}")}</span>
                                        <span class="pill pill-neutral">{format!("index · {index}")}</span>
                                        <span class="pill pill-neutral">{format!("{}d", profile.dimensions)}</span>
                                    </div>
                                </div>
                            }.into_any()
                        }
                    }
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn ChunkingConfigCard(
    default_id: Option<Uuid>,
    configs: Resource<Vec<ChunkingConfigurationDto>>,
) -> impl IntoView {
    view! {
        <div class="surface-raised rounded p-3">
            <div class="text-xs uppercase tracking-wide muted mb-2">"Chunking"</div>
            <Suspense fallback=|| view! { <div class="muted text-sm">"Loading…"</div> }>
                {move || configs.get().map(|list| {
                    let resolved = resolve_chunking_config(&list, default_id);
                    match resolved {
                        None => view! {
                            <div class="muted text-sm">"No chunking configuration available"</div>
                        }.into_any(),
                        Some((cfg, is_explicit)) => {
                            let descriptor = cfg.config.describe();
                            let source_pill = if is_explicit { "connector" } else { "app default" };
                            view! {
                                <div class="flex flex-col gap-2">
                                    <div class="flex items-center gap-2 min-w-0">
                                        <span class="font-medium truncate">{cfg.name.clone()}</span>
                                        <span class="pill pill-neutral text-xs">{source_pill}</span>
                                    </div>
                                    <div class="text-xs muted font-mono">{descriptor}</div>
                                </div>
                            }.into_any()
                        }
                    }
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn SyncRow(sync: ConnectorSyncSummaryDto) -> impl IntoView {
    let status_kind = match sync.status.as_str() {
        "completed" => Status::Ok,
        "failed" => Status::Fail,
        "started" => Status::Pending,
        _ => Status::Info,
    };
    let started_label = format_when(&sync.started_at);
    let started_title = sync.started_at.clone();
    let completed_label = sync
        .completed_at
        .as_deref()
        .map(format_when)
        .unwrap_or_else(|| "—".into());
    let completed_title = sync.completed_at.clone().unwrap_or_default();
    let discovered_count = sync.discovered_count;
    let error = sync.error.clone();

    view! {
        <div class="flex flex-col gap-2">
            <div class="flex items-center gap-4 flex-wrap text-sm">
                <StatusPill label=sync.status.clone() kind=status_kind />
                <span class="muted" title=started_title>{format!("Started {started_label}")}</span>
                <span class="muted" title=completed_title>{format!("Completed {completed_label}")}</span>
                <span class="muted">{format!("{discovered_count} discovered")}</span>
            </div>
            {error.map(|msg| view! {
                <div class="log-line-error text-xs">{msg}</div>
            })}
        </div>
    }
}

const HISTORY_PAGE_SIZE: u32 = 10;

#[component]
fn SyncHistoryDialog(
    connector_id: Signal<Uuid>,
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
) -> impl IntoView {
    let (page, set_page) = signal(0u32);

    Effect::new(move |_| {
        if open.get() {
            set_page.set(0);
        }
    });

    let history = Resource::new(
        move || (open.get(), connector_id.get(), page.get()),
        |(is_open, id, p)| async move {
            if !is_open {
                return Ok(Vec::new());
            }
            list_connector_syncs(id, HISTORY_PAGE_SIZE, p * HISTORY_PAGE_SIZE)
                .await
                .map_err(|e| e.to_string())
        },
    );

    let close = Callback::new(move |_| set_open.set(false));
    let on_prev = move |_| set_page.update(|p| *p = p.saturating_sub(1));
    let on_next = move |_| set_page.update(|p| *p += 1);

    view! {
        <Dialog
            open=Signal::derive(move || open.get())
            title="Sync history".to_string()
            subtitle="Paginated history of connector syncs, newest first.".to_string()
            on_close=close
        >
            <div class="flex flex-col gap-3">
                <Suspense fallback=|| view! { <div class="muted text-sm p-2">"Loading…"</div> }>
                    {move || history.get().map(|res| match res {
                        Err(e) => view! { <div class="log-line-error text-sm">{format!("Failed: {e}")}</div> }.into_any(),
                        Ok(list) if list.is_empty() && page.get() == 0 => view! {
                            <div class="muted text-sm">"No syncs yet"</div>
                        }.into_any(),
                        Ok(list) => view! {
                            <SyncHistoryTable rows=list />
                        }.into_any(),
                    })}
                </Suspense>

                <div class="flex items-center justify-between pt-2 border-t border-[var(--color-border)]">
                    <span class="text-sm muted">{move || format!("Page {}", page.get() + 1)}</span>
                    <div class="flex gap-2">
                        <button
                            type="button"
                            class="btn"
                            disabled=move || page.get() == 0
                            on:click=on_prev
                        >
                            "← Previous"
                        </button>
                        <button
                            type="button"
                            class="btn"
                            disabled=move || {
                                history.get()
                                    .and_then(Result::ok)
                                    .map(|l| (l.len() as u32) < HISTORY_PAGE_SIZE)
                                    .unwrap_or(true)
                            }
                            on:click=on_next
                        >
                            "Next →"
                        </button>
                    </div>
                </div>
            </div>
        </Dialog>
    }
}

#[component]
fn SyncHistoryTable(rows: Vec<ConnectorSyncSummaryDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th>"Started"</th>
                    <th>"Status"</th>
                    <th class="text-right">"Discovered"</th>
                    <th>"Completed"</th>
                </tr>
            </thead>
            <tbody>
                {rows.into_iter().map(|s| {
                    let status_kind = match s.status.as_str() {
                        "completed" => Status::Ok,
                        "failed" => Status::Fail,
                        "started" => Status::Pending,
                        _ => Status::Info,
                    };
                    let started = format_when(&s.started_at);
                    let completed = s
                        .completed_at
                        .as_deref()
                        .map(format_when)
                        .unwrap_or_else(|| "—".into());
                    view! {
                        <tr>
                            <td class="text-xs muted" title=s.started_at.clone()>{started}</td>
                            <td><StatusPill label=s.status.clone() kind=status_kind /></td>
                            <td class="text-right text-sm">{s.discovered_count.to_string()}</td>
                            <td class="text-xs muted" title=s.completed_at.clone().unwrap_or_default()>{completed}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }
}

fn resolve_index_profile(
    profiles: &[IndexProfileDto],
    explicit_id: Option<Uuid>,
) -> Option<(IndexProfileDto, bool)> {
    if let Some(id) = explicit_id {
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

fn resolve_chunking_config(
    configs: &[ChunkingConfigurationDto],
    explicit_id: Option<Uuid>,
) -> Option<(ChunkingConfigurationDto, bool)> {
    if let Some(id) = explicit_id {
        if let Some(c) = configs.iter().find(|c| c.chunking_configuration_id == id) {
            return Some((c.clone(), true));
        }
    }
    configs
        .iter()
        .find(|c| c.is_default)
        .cloned()
        .map(|c| (c, false))
}

fn short_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}
