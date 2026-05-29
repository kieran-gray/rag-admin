use leptos::prelude::*;
use leptos_router::components::A;

use crate::shared::contracts::{ActivityJobDto, ActivityKind, ActivityStatus};
use crate::ui::pages::shared::format_when;
use crate::ui::state::activity::{open_tray_with, use_activity_state};
use crate::ui::state::recent::{recent_locations, RecentLocation};

#[component]
pub(super) fn RecentPanel() -> impl IntoView {
    let activity = use_activity_state();
    let jobs = move || {
        activity
            .ordered_jobs()
            .into_iter()
            .take(5)
            .collect::<Vec<_>>()
    };

    let (locations, set_locations) = signal(Vec::<RecentLocation>::new());
    Effect::new(move |_| set_locations.set(recent_locations()));

    view! {
        <section class="surface p-4 space-y-4">
            <h2 class="section-title">"Recent"</h2>

            <div class="space-y-1.5">
                <div class="eyebrow">"Jobs"</div>
                {move || {
                    let list = jobs();
                    if list.is_empty() {
                        view! { <p class="text-xs muted py-1">"No recent jobs."</p> }.into_any()
                    } else {
                        view! {
                            <ul class="flex flex-col gap-1">
                                {list.iter().map(job_row).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
            </div>

            {move || {
                let list = locations.get();
                (!list.is_empty()).then(|| view! {
                    <div class="space-y-1.5">
                        <div class="eyebrow">"Pages"</div>
                        <ul class="flex flex-col gap-1">
                            {list.into_iter().take(5).map(|loc| view! {
                                <li>
                                    <A
                                        href=loc.path
                                        attr:class="flex items-center gap-3 rounded px-2 py-1.5 hover:bg-[var(--color-surface-hover)]"
                                    >
                                        <span class="faint w-5 text-center" aria-hidden="true">"›"</span>
                                        <span class="text-sm text-text truncate">{loc.title}</span>
                                    </A>
                                </li>
                            }).collect_view()}
                        </ul>
                    </div>
                })
            }}
        </section>
    }
}

fn job_row(job: &ActivityJobDto) -> impl IntoView {
    let id = job.stream_id;
    let kind = kind_label(job.kind);
    let when = format_when(job.finished_at.as_deref().unwrap_or(&job.started_at));
    let label = job.label.clone();
    let dot = status_dot(job.status);
    view! {
        <li>
            <button
                type="button"
                class="w-full text-left flex items-center gap-2 rounded px-2 py-1.5 hover:bg-[var(--color-surface-hover)]"
                on:click=move |_| open_tray_with(id)
            >
                <span class=dot></span>
                <span class="min-w-0 flex-1">
                    <span class="block text-sm text-text truncate">{label}</span>
                    <span class="block text-xs muted">{kind}" · "{when}</span>
                </span>
            </button>
        </li>
    }
}

fn kind_label(kind: ActivityKind) -> &'static str {
    match kind {
        ActivityKind::Indexing => "Indexing",
        ActivityKind::EvaluationDataset => "Dataset",
        ActivityKind::EvaluationRun => "Run",
        ActivityKind::DocumentMap => "Map",
        ActivityKind::ConnectorImport => "Import",
    }
}

fn status_dot(status: ActivityStatus) -> &'static str {
    match status {
        ActivityStatus::Running => "activity-dot activity-dot-active",
        ActivityStatus::Failed => "activity-dot activity-dot-fail",
        _ => "activity-dot",
    }
}
