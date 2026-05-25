use std::cmp::Ordering;

use leptos::either::Either;
use leptos::prelude::*;
use uuid::Uuid;

use crate::server_functions::evaluation::get_run_question_results;
use crate::shared::contracts::{RunQuestionResultDto, RunQuestionResultsDto};
use crate::shared::EvaluationResultSplit;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortBy {
    WorstRecall,
    Sequence,
}

#[component]
pub fn QuestionDrilldown(
    run_id: Uuid,
    variant_label: String,
    split: EvaluationResultSplit,
) -> impl IntoView {
    let (sort_by, set_sort_by) = signal(SortBy::WorstRecall);
    let (expanded, set_expanded) = signal::<Option<u32>>(None);
    let variant_label = StoredValue::new(variant_label);
    let split_value = StoredValue::new(split);

    let resource = Resource::new(
        move || (run_id, variant_label.get_value(), split_value.get_value()),
        move |(rid, vlabel, s)| async move {
            get_run_question_results(rid, vlabel, Some(s))
                .await
                .map_err(|e| e.to_string())
        },
    );

    view! {
        <div class="question-drilldown">
            <Transition fallback=|| view! {
                <div class="question-drilldown-loading muted text-xs">"Loading per-question…"</div>
            }>
                {move || resource.get().map(|res| match res {
                    Err(e) => Either::Left(view! {
                        <div class="log-line-error text-xs p-3">{format!("Failed to load: {e}")}</div>
                    }),
                    Ok(None) => Either::Right(Either::Left(view! {
                        <div class="muted text-xs p-3 italic">"No trace data for this split."</div>
                    })),
                    Ok(Some(data)) => Either::Right(Either::Right(view! {
                        <QuestionsBody
                            data=data
                            sort_by=sort_by
                            set_sort_by=set_sort_by
                            expanded=expanded
                            set_expanded=set_expanded
                        />
                    })),
                })}
            </Transition>
        </div>
    }
}

#[component]
fn QuestionsBody(
    data: RunQuestionResultsDto,
    sort_by: ReadSignal<SortBy>,
    set_sort_by: WriteSignal<SortBy>,
    expanded: ReadSignal<Option<u32>>,
    set_expanded: WriteSignal<Option<u32>>,
) -> impl IntoView {
    let count = data.questions.len();
    let rows = StoredValue::new(data.questions);

    let sorted = move || {
        let mut copy = rows.with_value(Clone::clone);
        sort(&mut copy, sort_by.get());
        copy
    };

    view! {
        <div class="question-drilldown-inner">
            <div class="question-drilldown-toolbar">
                <span class="text-xs muted">{format!("{count} questions")}</span>
                <div class="toolbar-spacer"></div>
                <div class="seg-toggle" role="tablist" aria-label="Sort">
                    <button
                        type="button"
                        role="tab"
                        class:is-active=move || sort_by.get() == SortBy::WorstRecall
                        aria-selected=move || (sort_by.get() == SortBy::WorstRecall).to_string()
                        on:click=move |_| set_sort_by.set(SortBy::WorstRecall)
                    >"Worst first"</button>
                    <button
                        type="button"
                        role="tab"
                        class:is-active=move || sort_by.get() == SortBy::Sequence
                        aria-selected=move || (sort_by.get() == SortBy::Sequence).to_string()
                        on:click=move |_| set_sort_by.set(SortBy::Sequence)
                    >"By #"</button>
                </div>
            </div>
            <div class="question-card-list">
                {move || sorted().into_iter().map(|q| {
                    let seq = q.sequence;
                    view! {
                        <QuestionCard
                            q=q
                            is_open=Signal::derive(move || expanded.get() == Some(seq))
                            on_toggle=Callback::new(move |_| {
                                set_expanded.update(|v| {
                                    *v = if *v == Some(seq) { None } else { Some(seq) };
                                });
                            })
                        />
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn QuestionCard(
    q: RunQuestionResultDto,
    is_open: Signal<bool>,
    on_toggle: Callback<()>,
) -> impl IntoView {
    let seq = q.sequence;
    let category = q.category.clone();
    let question = q.question.clone();
    let recall = q.recall;
    let precision = q.precision;
    let iou = q.iou;
    let refs_total = q.references.len();
    let refs_covered = q.references.iter().filter(|r| r.covered).count();
    let chunks_count = q.retrieved.len();
    let detail = StoredValue::new(q);

    view! {
        <div class="question-card" class:is-open=is_open>
            <button
                type="button"
                class="question-card-head"
                on:click=move |_| on_toggle.run(())
                aria-expanded=move || is_open.get().to_string()
            >
                <span class="question-card-seq">{format!("Q{}", seq + 1)}</span>
                <span class="question-card-text">{question}</span>
                <span class="question-card-meta">
                    {(!category.is_empty()).then(|| view! {
                        <span class="pill pill-neutral text-xs">{category.clone()}</span>
                    })}
                    <MetricChip label="R" value=recall />
                    <MetricChip label="P" value=precision />
                    <MetricChip label="IoU" value=iou />
                    <span class="question-card-counts">
                        {format!("refs {refs_covered}/{refs_total} · chunks {chunks_count}")}
                    </span>
                </span>
                <span class="question-card-chevron" aria-hidden="true">
                    {move || if is_open.get() { "▾" } else { "▸" }}
                </span>
            </button>
            <Show when=move || is_open.get() fallback=|| ()>
                <div class="question-card-body">
                    <QuestionDetail q=detail.get_value() />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn MetricChip(label: &'static str, value: f32) -> impl IntoView {
    let class = bucket_class(value);
    view! {
        <span class=format!("metric-chip {class}")>
            <span class="metric-chip-label">{label}</span>
            <span class="metric-chip-value">{format!("{:.0}%", value * 100.0)}</span>
        </span>
    }
}

#[component]
fn QuestionDetail(q: RunQuestionResultDto) -> impl IntoView {
    let references = q.references.clone();
    let retrieved = q.retrieved.clone();
    view! {
        <div class="question-detail">
            <div class="question-detail-section">
                <div class="eyebrow mb-2">{format!("Expected references · {}", references.len())}</div>
                {if references.is_empty() {
                    view! { <div class="muted text-xs italic">"No references recorded."</div> }.into_any()
                } else {
                    view! {
                        <ul class="reference-list">
                            {references.into_iter().map(|r| view! {
                                <li class="reference-row">
                                    <span class=if r.covered { "pill pill-ok text-xs" } else { "pill pill-fail text-xs" }>
                                        {if r.covered { "hit" } else { "miss" }}
                                    </span>
                                    <span class="muted font-mono text-xs reference-range">
                                        {format!("[{}–{}]", r.char_start, r.char_end)}
                                    </span>
                                    <span class="reference-content text-xs">{r.content}</span>
                                </li>
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }}
            </div>
            <div class="question-detail-section">
                <div class="eyebrow mb-2">{format!("Retrieved chunks · {}", retrieved.len())}</div>
                {if retrieved.is_empty() {
                    view! { <div class="muted text-xs italic">"Nothing retrieved."</div> }.into_any()
                } else {
                    view! {
                        <ol class="retrieved-list">
                            {retrieved.into_iter().map(|c| {
                                let heading_present = !c.heading.is_empty();
                                let text_present = !c.text.is_empty();
                                view! {
                                    <li class="retrieved-row">
                                        <div class="retrieved-row-head">
                                            <span class="retrieved-rank font-mono">{format!("#{}", c.rank + 1)}</span>
                                            <span class="retrieved-score font-mono">{format!("{:.2}", c.score)}</span>
                                            {c.is_reference_hit.then(|| view! {
                                                <span class="pill pill-ok text-xs">"hit"</span>
                                            })}
                                            {heading_present.then(|| view! {
                                                <span class="muted text-xs">{c.heading.clone()}</span>
                                            })}
                                            <span class="toolbar-spacer"></span>
                                            <span class="muted font-mono text-xs">
                                                {format!("[{}–{}]", c.char_start, c.char_end)}
                                            </span>
                                        </div>
                                        {text_present.then(|| view! {
                                            <p class="retrieved-text text-xs">{c.text.clone()}</p>
                                        })}
                                    </li>
                                }
                            }).collect_view()}
                        </ol>
                    }.into_any()
                }}
            </div>
        </div>
    }
}

fn sort(rows: &mut [RunQuestionResultDto], by: SortBy) {
    match by {
        SortBy::WorstRecall => rows.sort_by(|a, b| {
            a.recall
                .partial_cmp(&b.recall)
                .unwrap_or(Ordering::Equal)
                .then(
                    a.precision
                        .partial_cmp(&b.precision)
                        .unwrap_or(Ordering::Equal),
                )
                .then(a.sequence.cmp(&b.sequence))
        }),
        SortBy::Sequence => rows.sort_by_key(|r| r.sequence),
    }
}

fn bucket_class(value: f32) -> &'static str {
    if value >= 0.75 {
        "metric-good"
    } else if value >= 0.5 {
        "metric-ok"
    } else {
        "metric-poor"
    }
}
