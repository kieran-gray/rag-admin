use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_params_map};
use leptos_router::NavigateOptions;
use uuid::Uuid;

use crate::server_functions::evaluation::{
    cancel_dataset_generation, delete_dataset, get_dataset, rename_dataset,
};
use crate::shared::contracts::{
    aggregate_type, EvaluationDatasetDto, EvaluationQuestionDto, EvaluationReferenceDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    ConfirmDialog, EmptyState, Kv, PageHeader, Status, StatusPill, Surface,
};

use crate::ui::pages::shared::{format_when, short_hash};

#[component]
pub fn DatasetDetailPage() -> impl IntoView {
    let params = use_params_map();
    let dataset_id = Memo::new(move |_| {
        params
            .with(|p| p.get("dataset_id").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });

    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_DATASET]));
    let dataset = Resource::new(
        move || (dataset_id.get(), invalidator.get()),
        move |(id, _)| async move {
            match id {
                Some(id) => get_dataset(id).await.map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    view! {
        <Transition fallback=|| view! { <p class="muted">"Loading dataset…"</p> }>
            {move || dataset.get().map(|res| match res {
                Err(e) => view! {
                    <Surface><div class="log-line-error">{format!("Failed to load: {e}")}</div></Surface>
                }.into_any(),
                Ok(None) => view! {
                    <Surface>
                        <EmptyState
                            title="Dataset not found"
                            body="This dataset id is unknown or has been removed.".to_string()
                        />
                    </Surface>
                }.into_any(),
                Ok(Some(d)) => view! { <DatasetView dataset=d /> }.into_any(),
            })}
        </Transition>
    }
}

#[component]
fn DatasetView(dataset: EvaluationDatasetDto) -> impl IntoView {
    let (status_kind, status_label) = match dataset.status.as_str() {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "cancelled" => (Status::Cancel, "Cancelled"),
        "generating" => (Status::Pending, "Generating"),
        _ => (Status::Neutral, "Unknown"),
    };
    let is_generating = dataset.status == "generating";

    let dataset_id = dataset.dataset_id;
    let dataset_short = dataset_id.to_string().chars().take(8).collect::<String>();
    let document_id = dataset.document_id;
    let document_title = dataset.document_title.clone();
    let document_version = dataset.document_version;
    let content_hash_short = short_hash(&dataset.content_hash).to_string();
    let generation_model = dataset.generation_model.clone();
    let embedding_model_id = dataset.embedding_model_id.to_string();
    let target = dataset.target_question_count;
    let actual = dataset.question_count;
    let rejected = dataset.rejection_count;
    let created_at = format_when(&dataset.created_at);
    let failure_reason = dataset.failure_reason.clone();
    let questions = dataset.questions;
    let label = dataset.label;

    let eyebrow = match document_title.as_deref() {
        Some(title) if !title.is_empty() => format!("Evaluations / Dataset · {title}"),
        _ => "Evaluations / Dataset".to_string(),
    };

    let (editing, set_editing) = signal(false);
    let (label_input, set_label_input) = signal(label.clone());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (confirm_delete_open, set_confirm_delete_open) = signal(false);
    let (confirm_cancel_open, set_confirm_cancel_open) = signal(false);
    let original_label = StoredValue::new(label);

    let on_save_rename = StoredValue::new(move |_: leptos::ev::MouseEvent| {
        if busy.get_untracked() {
            return;
        }
        let new_label = label_input.get_untracked();
        if new_label.trim().is_empty() {
            set_error.set(Some("Label must not be empty.".to_string()));
            return;
        }
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match rename_dataset(dataset_id, new_label).await {
                Ok(_) => {
                    set_busy.set(false);
                    set_editing.set(false);
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    });

    let on_cancel_rename = StoredValue::new(move |_: leptos::ev::MouseEvent| {
        set_label_input.set(original_label.get_value());
        set_editing.set(false);
        set_error.set(None);
    });

    let open_confirm_cancel = Callback::new(move |_| {
        set_confirm_cancel_open.set(true);
        set_error.set(None);
    });
    let close_confirm_cancel = Callback::new(move |_| set_confirm_cancel_open.set(false));
    let confirm_cancel_generation = Callback::new(move |_| {
        if busy.get_untracked() {
            return;
        }
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match cancel_dataset_generation(dataset_id).await {
                Ok(_) => {
                    set_busy.set(false);
                    set_confirm_cancel_open.set(false);
                }
                Err(e) => {
                    set_busy.set(false);
                    set_confirm_cancel_open.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    });

    let open_confirm_delete = Callback::new(move |_| {
        set_confirm_delete_open.set(true);
        set_error.set(None);
    });
    let close_confirm_delete = Callback::new(move |_| set_confirm_delete_open.set(false));
    let confirm_delete_action = Callback::new(move |_| {
        if busy.get_untracked() {
            return;
        }
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match delete_dataset(dataset_id).await {
                Ok(_) => {
                    use_navigate()(
                        "/artifacts/datasets",
                        NavigateOptions {
                            replace: true,
                            ..Default::default()
                        },
                    );
                }
                Err(e) => {
                    set_busy.set(false);
                    set_confirm_delete_open.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    });

    view! {
        <div>
            <PageHeader
                title=format!("Dataset {dataset_short}")
                eyebrow=eyebrow
                subtitle=format!("{actual}/{target} questions · {created_at}")
                actions=Box::new(move || view! {
                    <StatusPill label=status_label.to_string() kind=status_kind />
                }.into_any())
            />

            <div class="mb-4 flex items-center gap-3 flex-wrap text-sm">
                <A href=format!("/evaluate/by-id/{document_id}") attr:class="muted hover:text-text">
                    "← Back to evaluation"
                </A>
                <A href=format!("/documents/by-id/{document_id}?tab=source") attr:class="muted hover:text-text">
                    "Open source document"
                </A>
            </div>

            <Surface class="mb-4".to_string()>
                <div class="flex items-center justify-between gap-3">
                    <div class="flex-1 min-w-0">
                        <div class="eyebrow mb-1">"Label"</div>
                        {move || if editing.get() {
                            view! {
                                <div class="flex items-center gap-2">
                                    <input
                                        class="input flex-1"
                                        prop:value=move || label_input.get()
                                        on:input=move |ev| set_label_input.set(event_target_value(&ev))
                                    />
                                    <button
                                        type="button"
                                        class="btn btn-primary"
                                        disabled=move || busy.get()
                                        on:click=move |ev| on_save_rename.with_value(|f| f(ev))
                                    >
                                        {move || if busy.get() { "Saving…" } else { "Save" }}
                                    </button>
                                    <button
                                        type="button"
                                        class="btn"
                                        disabled=move || busy.get()
                                        on:click=move |ev| on_cancel_rename.with_value(|f| f(ev))
                                    >
                                        "Cancel"
                                    </button>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="flex items-center gap-3">
                                    <span class="text-text text-base truncate">{label_input.get()}</span>
                                    <button
                                        type="button"
                                        class="btn btn-ghost text-xs"
                                        on:click=move |_| { set_editing.set(true); set_error.set(None); }
                                    >
                                        "Rename"
                                    </button>
                                </div>
                            }.into_any()
                        }}
                    </div>
                    <div class="shrink-0 flex items-center gap-2">
                        {is_generating.then(|| view! {
                            <button
                                type="button"
                                class="btn"
                                on:click=move |_| open_confirm_cancel.run(())
                            >
                                "Cancel generation"
                            </button>
                        })}
                        <button
                            type="button"
                            class="btn btn-danger"
                            on:click=move |_| open_confirm_delete.run(())
                        >
                            "Delete"
                        </button>
                    </div>
                </div>
                {move || error.get().map(|e| view! {
                    <div class="log-line-error text-sm mt-2">{e}</div>
                })}
            </Surface>

            {failure_reason.map(|reason| view! {
                <Surface class="mb-4 border-l-2 border-l-[var(--color-fail)]".to_string()>
                    <div class="log-line-error text-sm">{reason}</div>
                </Surface>
            })}

            <details class="surface collapsible-card mb-4">
                <summary class="collapsible-card-summary">
                    <div class="section-title">"Generation context"</div>
                    <span class="collapsible-card-hint muted text-xs">
                        <span>{generation_model.clone()}</span>
                        <span class="collapsible-card-chevron">"▾"</span>
                    </span>
                </summary>
                <div class="collapsible-card-body">
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-y-1.5 gap-x-6">
                        <Kv label="Document".to_string() value=document_id.to_string() />
                        <Kv label="Version".to_string() value=format!("v{document_version} · {content_hash_short}") />
                        <Kv label="Generation model".to_string() value=generation_model.clone() />
                        <Kv label="Embedding model".to_string() value=embedding_model_id />
                        <Kv label="Rejections".to_string() value=format!("{rejected}") />
                    </div>
                </div>
            </details>

            <Surface title=format!("Questions ({})", questions.len())>
                {if questions.is_empty() {
                    view! {
                        <EmptyState
                            title="No questions yet"
                            body="Generation may still be in progress; questions land here as they're accepted.".to_string()
                        />
                    }.into_any()
                } else {
                    let dimension_summary = summarise_dimensions(&questions);
                    view! {
                        <DimensionDistribution summary=dimension_summary />
                        <div class="space-y-2 mt-4">
                            {questions.into_iter().map(|q| view! {
                                <QuestionCard question=q />
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </Surface>

            <ConfirmDialog
                open=confirm_delete_open
                title="Delete dataset".to_string()
                message="This dataset and all of its questions will be permanently removed.".to_string()
                confirm_label="Delete dataset".to_string()
                confirm_busy_label="Deleting…".to_string()
                busy=busy
                danger=true
                on_confirm=confirm_delete_action
                on_close=close_confirm_delete
            />

            <ConfirmDialog
                open=confirm_cancel_open
                title="Cancel generation".to_string()
                message="Stop generating questions for this dataset? Already-accepted questions will be kept.".to_string()
                confirm_label="Cancel generation".to_string()
                confirm_busy_label="Cancelling…".to_string()
                busy=busy
                on_confirm=confirm_cancel_generation
                on_close=close_confirm_cancel
            />
        </div>
    }
}

#[component]
fn QuestionCard(question: EvaluationQuestionDto) -> impl IntoView {
    let references = question.references;
    let ref_count = references.len();
    let question_text = question.question;
    let sequence = question.sequence;
    let operation = question.dimensions.operation.clone();
    let evidence = question.dimensions.evidence.clone();
    let role = question.dimensions.role.clone();

    view! {
        <details class="surface-raised rounded collapsible-card">
            <summary class="collapsible-card-summary">
                <div class="flex items-start gap-3 flex-wrap min-w-0 flex-1">
                    <span class="font-mono text-xs muted shrink-0 pt-0.5">{format!("Q{sequence:02}")}</span>
                    <p class="text-text leading-relaxed flex-1 min-w-0">{question_text}</p>
                    <div class="flex items-center gap-1 shrink-0">
                        <span class="pill pill-neutral text-xs" title="Cognitive operation">{operation}</span>
                        <span class="pill pill-neutral text-xs" title="Evidence tier">{evidence}</span>
                        <span class="pill pill-neutral text-xs" title="Reader role">{role}</span>
                    </div>
                </div>
                <span class="collapsible-card-hint muted text-xs shrink-0">
                    <span>{format!("{ref_count} ref{}", if ref_count == 1 { "" } else { "s" })}</span>
                    <span class="collapsible-card-chevron">"▾"</span>
                </span>
            </summary>
            <div class="collapsible-card-body">
                {if references.is_empty() {
                    view! { <p class="text-xs muted">"No references attached."</p> }.into_any()
                } else {
                    view! {
                        <div class="space-y-2">
                            {references.into_iter().enumerate().map(|(i, r)| view! {
                                <ReferenceCard index=(i as u32) + 1 reference=r />
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>
        </details>
    }
}

#[component]
fn ReferenceCard(index: u32, reference: EvaluationReferenceDto) -> impl IntoView {
    let span = format!("[{}–{}]", reference.char_start, reference.char_end);
    let content = reference.content;
    let href = format!(
        "/documents/by-id/{}?tab=source&ref_start={}&ref_end={}",
        reference.document_id, reference.char_start, reference.char_end,
    );
    view! {
        <A href=href attr:class="block border-l-2 border-[var(--color-border)] pl-3 py-1 hover:border-[var(--color-accent)] no-underline">
            <div class="flex items-center gap-2 mb-1">
                <span class="font-mono text-xs muted">{format!("ref {index}")}</span>
                <span class="font-mono text-xs faint">{span}</span>
                <span class="ml-auto text-xs muted">"Open in source ↗"</span>
            </div>
            <p class="text-sm text-text whitespace-pre-wrap">{content}</p>
        </A>
    }
}

#[derive(Clone)]
struct DimensionSummary {
    operation: Vec<(String, u32)>,
    evidence: Vec<(String, u32)>,
    role: Vec<(String, u32)>,
}

fn summarise_dimensions(questions: &[EvaluationQuestionDto]) -> DimensionSummary {
    fn tally<F: Fn(&EvaluationQuestionDto) -> &str>(
        questions: &[EvaluationQuestionDto],
        key: F,
    ) -> Vec<(String, u32)> {
        let mut counts: Vec<(String, u32)> = Vec::new();
        for q in questions {
            let value = key(q);
            if let Some(entry) = counts.iter_mut().find(|(name, _)| name == value) {
                entry.1 += 1;
            } else {
                counts.push((value.to_string(), 1));
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    }

    DimensionSummary {
        operation: tally(questions, |q| q.dimensions.operation.as_str()),
        evidence: tally(questions, |q| q.dimensions.evidence.as_str()),
        role: tally(questions, |q| q.dimensions.role.as_str()),
    }
}

#[component]
fn DimensionDistribution(summary: DimensionSummary) -> impl IntoView {
    let row = move |label: &'static str, items: Vec<(String, u32)>| {
        view! {
            <div class="grid grid-cols-[6rem_1fr] gap-3 items-baseline">
                <span class="eyebrow">{label}</span>
                <div class="flex flex-wrap items-center gap-1">
                    {items.into_iter().map(|(name, count)| view! {
                        <span class="pill pill-neutral text-xs">
                            <span>{name}</span>
                            <span class="muted">{format!("· {count}")}</span>
                        </span>
                    }).collect_view()}
                </div>
            </div>
        }
    };
    view! {
        <div class="space-y-2 pb-3 border-b border-[var(--color-border)]">
            {row("operation", summary.operation)}
            {row("evidence", summary.evidence)}
            {row("role", summary.role)}
        </div>
    }
}
