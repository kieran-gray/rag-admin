use leptos::either::Either;
use leptos::prelude::*;
use leptos_router::components::A;
use uuid::Uuid;

use crate::server_functions::comprehension::list_document_maps;
use crate::shared::contracts::DocumentMapSummaryDto;
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, StatusPill, Surface};
use crate::ui::pages::document_detail::map_phase::{section_count, MapPhase, MapProgress};

#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn MapStep(
    document_id: Uuid,
    document_version: u32,
    document_type: String,
    source_ref_key: String,
    on_advance: Callback<()>,
) -> impl IntoView {
    let map_invalidator = use_invalidator(|e| e.from_any(&["document_map"]));
    let maps = Resource::new(
        move || (document_id, map_invalidator.get()),
        move |(id, _)| async move { list_document_maps(id).await.map_err(|e| e.to_string()) },
    );

    let map_href = format!(
        "/documents/{}/{}/map",
        document_type,
        urlencoding::encode(&source_ref_key),
    );
    let map_href_panel = map_href.clone();

    let current_phase = move || {
        maps.get().and_then(Result::ok).and_then(|list| {
            list.into_iter()
                .find(|m| m.document_version == document_version)
                .map(|m| MapPhase::from_status(&m.status))
        })
    };

    let advance_enabled = move || current_phase().is_some_and(MapPhase::is_ready);

    view! {
        <div class="space-y-4">
            <Surface title="Map".to_string()>
                <Transition fallback=|| view! { <p class="muted text-sm">"Loading map…"</p> }>
                    {move || {
                        let href = map_href_panel.clone();
                        maps.get().map(|res| match res {
                            Err(e) => Either::Left(view! {
                                <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                            }),
                            Ok(list) => Either::Right({
                                let current = list.into_iter()
                                    .find(|m| m.document_version == document_version);
                                match current {
                                    Some(summary) => Either::Left(view! {
                                        <MapStepReady summary=summary href=href />
                                    }),
                                    None => Either::Right(view! {
                                        <MapStepMissing href=href />
                                    }),
                                }
                            }),
                        })
                    }}
                </Transition>
            </Surface>

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Next"</span>
                    <span class="step-advance-eyebrow-label">"Pick or generate a dataset"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=move || !advance_enabled()
                    on:click=move |_| on_advance.run(())
                >
                    {move || if advance_enabled() {
                        "Continue to dataset →"
                    } else {
                        "Build a map to continue"
                    }}
                </button>
            </div>
        </div>
    }
}

#[component]
fn MapStepReady(summary: DocumentMapSummaryDto, href: String) -> impl IntoView {
    let phase = MapPhase::from_status(&summary.status);
    let (status_kind, status_label) = phase.pill(MapProgress {
        observations_extracted: summary.observations_extracted,
        chunk_count: summary.chunk_count,
        threads_synthesized: summary.threads_synthesized,
        section_size: summary.section_size,
    });
    let sections = section_count(summary.chunk_count, summary.section_size);
    let tier_counts = format!(
        "{} insight{} · {} thread{} · {} observation{}",
        summary.insights_synthesized,
        plural(summary.insights_synthesized),
        summary.threads_synthesized,
        plural(summary.threads_synthesized),
        summary.observations_extracted,
        plural(summary.observations_extracted),
    );
    let meta_line = format!(
        "{} chunk{} · {} section{}",
        summary.chunk_count,
        plural(summary.chunk_count),
        sections,
        plural(sections),
    );
    let show_progress = !phase.is_ready() && !phase.is_failed();
    let failure_reason = summary.failure_reason.clone();
    let link_label = if show_progress {
        "View progress →"
    } else {
        "Open map →"
    };

    view! {
        <div class="flex flex-col gap-2">
            <div class="flex items-center justify-between gap-3 flex-wrap">
                <StatusPill label=status_label kind=status_kind />
                <A href=href attr:class="btn btn-ghost btn-compact">{link_label}</A>
            </div>

            {(!show_progress).then(|| view! {
                <div class="text-sm text-[var(--color-text)] font-mono">{tier_counts.clone()}</div>
            })}

            <div class="text-[11px] muted font-mono">{meta_line}</div>

            {failure_reason.map(|r| view! {
                <div class="log-line-error text-xs">{r}</div>
            })}
        </div>
    }
}

#[component]
fn MapStepMissing(href: String) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-3">
            <EmptyState
                title="No map yet"
                body="A map extracts observations, threads, and insights from the document. Dataset generation uses these as seeds for evaluation questions.".to_string()
            />
            <div>
                <A href=href attr:class="btn btn-primary">
                    "Build map"
                </A>
            </div>
        </div>
    }
}

fn plural(n: u32) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
