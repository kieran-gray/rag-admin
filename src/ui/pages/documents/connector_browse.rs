use std::collections::HashSet;

use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::connector::list_connectors;
use crate::server_functions::connector_sync::{
    bulk_import_from_connector, list_connector_discovered, list_connector_syncs, run_connector_sync,
};
use crate::shared::contracts::{
    aggregate_type, ConnectorDiscoveredItemViewDto, ConnectorDto, ConnectorItemStatusDto,
    ConnectorSyncSummaryDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    Help, InlineStatus, InlineStatusMessage, Status, StatusPill, Surface, TitleCell,
};
use crate::ui::pages::shared::format_when;

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

    let syncs = Resource::new(
        move || (connector_id.get(), refresh.get(), invalidator.get()),
        |(id, _, _)| async move {
            list_connector_syncs(id, 10)
                .await
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
                            syncs=syncs
                            discovered=discovered
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
    syncs: Resource<Result<Vec<ConnectorSyncSummaryDto>, String>>,
    discovered: Resource<Result<Vec<ConnectorDiscoveredItemViewDto>, String>>,
    busy: ReadSignal<bool>,
    status: ReadSignal<Option<InlineStatusMessage>>,
    selected: ReadSignal<HashSet<String>>,
    set_selected: WriteSignal<HashSet<String>>,
    on_sync_now: Callback<()>,
    on_import_selected: Callback<bool>,
    on_import_all_new: Callback<()>,
) -> impl IntoView {
    let pipeline_id = connector.default_pipeline_configuration_id;
    let chunking_id = connector.default_chunking_configuration_id;

    view! {
        <div class="flex flex-col gap-3">
            <div class="flex items-center justify-between gap-3">
                <div class="text-sm muted">
                    "Bulk import items discovered from this connector. Defaults to the connector's pipeline and chunking when indexing."
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=busy
                    on:click=move |_| on_sync_now.run(())
                >
                    {move || if busy.get() { "Syncing…" } else { "Sync now" }}
                </button>
            </div>

            <InlineStatus status=status />


            <Surface>
                <div class="p-4 flex flex-col gap-2">
                    <div class="text-xs uppercase tracking-wide muted">"Ingestion defaults"</div>
                    <div class="flex gap-2 flex-wrap text-sm">
                        <span class="pill pill-neutral">
                            {pipeline_id
                                .map(|id| format!("Pipeline: {id}"))
                                .unwrap_or_else(|| "Pipeline: app default".into())}
                        </span>
                        <span class="pill pill-neutral">
                            {chunking_id
                                .map(|id| format!("Chunking: {id}"))
                                .unwrap_or_else(|| "Chunking: app default".into())}
                        </span>
                    </div>
                </div>
            </Surface>

            <Surface>
                <div class="p-4">
                    <h3 class="section-title">"Recent syncs"</h3>
                    <Suspense fallback=|| view! { <div class="muted text-sm p-2">"Loading…"</div> }>
                        {move || syncs.get().map(|res| match res {
                            Err(e) => view! { <div class="log-line-error text-sm">{format!("Failed: {e}")}</div> }.into_any(),
                            Ok(list) if list.is_empty() => view! { <div class="muted text-sm">"No syncs yet"</div> }.into_any(),
                            Ok(list) => view! {
                                <table class="data-table">
                                    <thead>
                                        <tr><th>"Started"</th><th>"Status"</th><th class="text-right">"Discovered"</th><th>"Completed"</th></tr>
                                    </thead>
                                    <tbody>
                                        {list.into_iter().map(|s| {
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
                            }.into_any(),
                        })}
                    </Suspense>
                </div>
            </Surface>

            <Surface>
                <div class="p-4 flex flex-col gap-3">
                    <div class="flex items-center justify-between flex-wrap gap-2">
                        <div class="flex items-center gap-1">
                            <h3 class="section-title">"Discovered items"</h3>
                            <Help title="Status meanings".to_string() label="What do these statuses mean?".to_string()>
                                <StatusLegend />
                            </Help>
                        </div>
                        <div class="flex gap-2">
                            <button
                                type="button"
                                class="btn"
                                disabled=move || busy.get() || selected.get().is_empty()
                                on:click=move |_| on_import_selected.run(false)
                                title="Store the markdown locally without adding it to a vector index"
                            >
                                "Import selected"
                            </button>
                            <button
                                type="button"
                                class="btn btn-primary"
                                disabled=move || busy.get() || selected.get().is_empty()
                                on:click=move |_| on_import_selected.run(true)
                                title="Import and then chunk + embed into this connector's pipeline"
                            >
                                "Import + index selected"
                            </button>
                            <button
                                type="button"
                                class="btn"
                                disabled=busy
                                on:click=move |_| on_import_all_new.run(())
                            >
                                "Import all new (+ index)"
                            </button>
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
                "Use "<strong>"Import selected"</strong>" to just store the content. Use "<strong>"Import + index selected"</strong>" to also chunk + embed into this connector's configured pipeline."
            </p>
        </div>
    }
}
