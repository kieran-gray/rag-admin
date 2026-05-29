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
    bulk_import_from_connector, list_connector_discovered, list_connector_syncs, run_connector_sync,
};
use crate::shared::contracts::{
    aggregate_type, BulkImportResultDto, ConnectorDiscoveredItemViewDto, ConnectorItemStatusDto,
    ConnectorSyncSummaryDto, IndexProfileDto,
};
use crate::ui::components::primitives::{
    ActionItem, ActionsMenu, Dialog, EmptyState, InlineStatus, InlineStatusMessage, PageHeader,
    Status, StatusPill, Surface, TitleCell,
};
use crate::ui::pages::configuration::connectors::form_dialog::{ConnectorForm, ConnectorFormDialog};
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
        move || (connector_id.get(), refresh.get(), invalidator.get()),
        |(id, _, _)| async move {
            list_connectors()
                .await
                .ok()
                .and_then(|list| list.into_iter().find(|c| c.connector_id == id))
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
    let (form, set_form) = signal::<Option<ConnectorForm>>(None);
    let (history_open, set_history_open) = signal(false);

    let open_edit = move |_| {
        if let Some(c) = connector.get_untracked().flatten() {
            set_form.set(Some(ConnectorForm::Edit(c)));
        }
    };

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
                            <button type="button" class="btn" disabled=busy on:click=open_edit>
                                "Edit"
                            </button>
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

                <Surface>
                    <div class="p-4 flex flex-col gap-3">
                        <div class="flex items-center justify-between flex-wrap gap-2">
                            <h3 class="section-title">"Most recent sync"</h3>
                            <button
                                type="button"
                                class="btn"
                                on:click=move |_| set_history_open.set(true)
                            >
                                "View history"
                            </button>
                        </div>
                        <Transition fallback=|| view! { <div class="muted text-sm p-2">"Loading…"</div> }>
                            {move || latest_sync.get().map(|res| match res {
                                Err(e) => view! { <div class="log-line-error text-sm">{format!("Failed: {e}")}</div> }.into_any(),
                                Ok(None) => view! { <div class="muted text-sm">"No syncs yet"</div> }.into_any(),
                                Ok(Some(sync)) => view! { <SyncRow sync=sync /> }.into_any(),
                            })}
                        </Transition>
                    </div>
                </Surface>

                <SyncHistoryDialog
                    connector_id=Signal::derive(move || connector_id.get())
                    open=history_open
                    set_open=set_history_open
                />

                <ConnectorFormDialog
                    form=form
                    set_form=set_form
                    busy=busy
                    set_busy=set_busy
                    set_status=set_status
                    set_refresh=set_refresh
                />

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

fn sync_status_kind(status: &str) -> Status {
    match status {
        "completed" => Status::Ok,
        "failed" => Status::Fail,
        "started" => Status::Pending,
        _ => Status::Info,
    }
}

#[component]
fn SyncRow(sync: ConnectorSyncSummaryDto) -> impl IntoView {
    let status_kind = sync_status_kind(&sync.status);
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
                    let status_kind = sync_status_kind(&s.status);
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
