use leptos::prelude::*;

use crate::components::activity::{toggle_drawer, use_activity_state};
use crate::components::log_stream::LogStream;
use crate::shared::{ActivityJobDto, ActivityKind, ActivityStatus};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    Running,
    Recent,
    Failed,
}

#[component]
pub fn ActivityDrawer() -> impl IntoView {
    let state = use_activity_state();
    let open = state.open;
    let rows = state.rows;
    let (filter, set_filter) = signal(Filter::Running);
    let (expanded, set_expanded) = signal::<Option<uuid::Uuid>>(None);

    view! {
        {move || open.get().then(|| view! {
            <div
                class="activity-backdrop"
                on:click=move |_| toggle_drawer(false)
            />
            <aside class="activity-drawer" aria-label="Activity drawer">
                <div class="activity-drawer-header">
                    <span class="activity-drawer-title">"Activity"</span>
                    <button
                        type="button"
                        class="btn-ghost btn"
                        on:click=move |_| toggle_drawer(false)
                        aria-label="Close drawer"
                        title="Close"
                    >"×"</button>
                </div>
                <div class="activity-drawer-filters">
                    <FilterButton
                        label="Running"
                        active=Signal::derive(move || filter.get() == Filter::Running)
                        on_click=Box::new(move || set_filter.set(Filter::Running))
                    />
                    <FilterButton
                        label="Recent"
                        active=Signal::derive(move || filter.get() == Filter::Recent)
                        on_click=Box::new(move || set_filter.set(Filter::Recent))
                    />
                    <FilterButton
                        label="Failed"
                        active=Signal::derive(move || filter.get() == Filter::Failed)
                        on_click=Box::new(move || set_filter.set(Filter::Failed))
                    />
                </div>
                <div class="activity-drawer-body">
                    {move || {
                        let f = filter.get();
                        let mut list = rows.get();
                        list.retain(|row| row_matches_filter(row, f));

                        list.sort_by(|a, b| match (a.status, b.status) {
                            (ActivityStatus::Running, ActivityStatus::Running) => {
                                a.started_at.cmp(&b.started_at)
                            }
                            (ActivityStatus::Running, _) => std::cmp::Ordering::Less,
                            (_, ActivityStatus::Running) => std::cmp::Ordering::Greater,
                            _ => b.finished_at.cmp(&a.finished_at),
                        });

                        if list.is_empty() {
                            return view! {
                                <div class="activity-empty">{empty_label(f)}</div>
                            }.into_any();
                        }

                        list.into_iter().map(|row| {
                            let id = row.stream_id;
                            view! {
                                <ActivityRowView
                                    row=row
                                    is_open=Signal::derive(move || expanded.get() == Some(id))
                                    on_toggle=move || {
                                        set_expanded.update(|cur| {
                                            *cur = if *cur == Some(id) { None } else { Some(id) };
                                        });
                                    }
                                />
                            }
                        }).collect_view().into_any()
                    }}
                </div>
            </aside>
        })}
    }
}

#[component]
fn FilterButton(
    label: &'static str,
    #[prop(into)] active: Signal<bool>,
    on_click: Box<dyn Fn() + Send + Sync>,
) -> impl IntoView {
    let on_click = StoredValue::new(on_click);
    view! {
        <button
            type="button"
            class=move || format!(
                "activity-drawer-filter {}",
                if active.get() { "is-active" } else { "" }
            )
            on:click=move |_| on_click.with_value(|f| f())
        >{label}</button>
    }
}

#[component]
fn ActivityRowView(
    row: ActivityJobDto,
    #[prop(into)] is_open: Signal<bool>,
    on_toggle: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    let stream_url = row.stream_url.clone();
    let on_toggle_stored = StoredValue::new(on_toggle);

    let status = row.status;
    let kind_label = match row.kind {
        ActivityKind::Indexing => "Indexing",
        ActivityKind::EvaluationDataset => "Dataset",
        ActivityKind::EvaluationRun => "Run",
    };
    let label = row.label.clone();
    let timestamp = row
        .finished_at
        .clone()
        .unwrap_or_else(|| row.started_at.clone());
    let timestamp = short_time(&timestamp);
    let status_dot_class = match status {
        ActivityStatus::Running => "activity-dot activity-dot-active",
        ActivityStatus::Completed => "activity-dot",
        ActivityStatus::Failed => "activity-dot activity-dot-fail",
    };

    view! {
        <div class="activity-row">
            <div
                class="activity-row-summary"
                on:click=move |_| on_toggle_stored.with_value(|f| f())
            >
                <span class=status_dot_class></span>
                <span class="activity-row-label">{label}</span>
                <span class="activity-row-meta">{kind_label}</span>
                <span class="activity-row-meta font-mono">{timestamp}</span>
            </div>
            {move || is_open.get().then(|| {
                let url = stream_url.clone();
                view! {
                    <div class="activity-row-detail">
                        {match url {
                            Some(u) => view! {
                                <LogStream url=Signal::derive(move || Some(u.clone())) />
                            }.into_any(),
                            None => view! {
                                <div class="muted text-xs italic">
                                    "No streamed log for this job — the drawer renders aggregate events on the bus."
                                </div>
                            }.into_any(),
                        }}
                    </div>
                }
            })}
        </div>
    }
}

fn row_matches_filter(row: &ActivityJobDto, filter: Filter) -> bool {
    match filter {
        Filter::Running => row.status == ActivityStatus::Running,
        Filter::Recent => row.status != ActivityStatus::Running,
        Filter::Failed => row.status == ActivityStatus::Failed,
    }
}

fn empty_label(filter: Filter) -> &'static str {
    match filter {
        Filter::Running => "Nothing running.",
        Filter::Recent => "No recent jobs.",
        Filter::Failed => "No failed jobs.",
    }
}

fn short_time(ts: &str) -> String {
    if ts.len() >= 16 {
        ts[..16].replace('T', " ")
    } else {
        ts.to_string()
    }
}
