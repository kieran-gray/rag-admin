use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_params_map};
use leptos_router::NavigateOptions;
use uuid::Uuid;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{EmptyState, Kv, PageHeader, Status, StatusPill, Surface};
use crate::server_functions::evaluation::{delete_dataset, get_dataset, rename_dataset};
use crate::shared::{
    aggregate_type, EvaluationDatasetDto, EvaluationQuestionDto, EvaluationReferenceDto,
};

use super::utils::short_hash;

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
        "generating" => (Status::Pending, "Generating"),
        _ => (Status::Neutral, "Unknown"),
    };

    let dataset_id = dataset.dataset_id;
    let dataset_short = dataset_id.to_string()[..8].to_string();
    let document_id = dataset.document_id;
    let document_version = dataset.document_version;
    let content_hash_short = short_hash(&dataset.content_hash).to_string();
    let generation_model = dataset.generation_model.clone();
    let generation_model_id = dataset.generation_model_id.to_string();
    let embedding_model_id = dataset.embedding_model_id.to_string();
    let target = dataset.target_question_count;
    let actual = dataset.question_count;
    let rejected = dataset.rejection_count;
    let created_at = dataset.created_at.clone();
    let failure_reason = dataset.failure_reason.clone();
    let questions = dataset.questions;
    let label = dataset.label;

    let (editing, set_editing) = signal(false);
    let (label_input, set_label_input) = signal(label.clone());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (confirm_delete, set_confirm_delete) = signal(false);
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

    let on_request_delete = StoredValue::new(move |_: leptos::ev::MouseEvent| {
        set_confirm_delete.set(true);
        set_error.set(None);
    });
    let on_cancel_delete =
        StoredValue::new(move |_: leptos::ev::MouseEvent| set_confirm_delete.set(false));
    let on_confirm_delete = StoredValue::new(move |_: leptos::ev::MouseEvent| {
        if busy.get_untracked() {
            return;
        }
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match delete_dataset(dataset_id).await {
                Ok(_) => {
                    use_navigate()(
                        &format!("/documents/by-id/{document_id}"),
                        NavigateOptions {
                            replace: true,
                            ..Default::default()
                        },
                    );
                }
                Err(e) => {
                    set_busy.set(false);
                    set_confirm_delete.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    });

    view! {
        <div>
            <PageHeader
                title=format!("Dataset {dataset_short}")
                eyebrow="Evaluations / Dataset".to_string()
                subtitle=format!("{actual}/{target} questions · {created_at}")
                actions=Box::new(move || view! {
                    <StatusPill label=status_label.to_string() kind=status_kind />
                }.into_any())
            />

            <div class="mb-4">
                <A href=format!("/documents/by-id/{document_id}") attr:class="muted text-sm">
                    "← Back to document"
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
                    <div class="shrink-0">
                        {move || if confirm_delete.get() {
                            view! {
                                <div class="flex items-center gap-2">
                                    <span class="text-xs muted">"Delete this dataset?"</span>
                                    <button
                                        type="button"
                                        class="btn btn-danger"
                                        disabled=move || busy.get()
                                        on:click=move |ev| on_confirm_delete.with_value(|f| f(ev))
                                    >
                                        {move || if busy.get() { "Deleting…" } else { "Confirm delete" }}
                                    </button>
                                    <button
                                        type="button"
                                        class="btn"
                                        disabled=move || busy.get()
                                        on:click=move |ev| on_cancel_delete.with_value(|f| f(ev))
                                    >
                                        "Cancel"
                                    </button>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <button
                                    type="button"
                                    class="btn btn-danger"
                                    on:click=move |ev| on_request_delete.with_value(|f| f(ev))
                                >
                                    "Delete"
                                </button>
                            }.into_any()
                        }}
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

            <Surface title="Generation context".to_string() class="mb-4".to_string()>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-y-1.5 gap-x-6">
                    <Kv label="Document".to_string() value=document_id.to_string() />
                    <Kv label="Version".to_string() value=format!("v{document_version} · {content_hash_short}") />
                    <Kv label="Generation model".to_string() value=format!("{generation_model} ({generation_model_id})") />
                    <Kv label="Embedding model".to_string() value=embedding_model_id />
                    <Kv label="Rejections".to_string() value=format!("{rejected}") />
                </div>
            </Surface>

            <Surface title=format!("Questions ({})", questions.len())>
                {if questions.is_empty() {
                    view! {
                        <EmptyState
                            title="No questions yet"
                            body="Generation may still be in progress; questions land here as they're accepted.".to_string()
                        />
                    }.into_any()
                } else {
                    view! {
                        <div class="space-y-3">
                            {questions.into_iter().enumerate().map(|(i, q)| view! {
                                <QuestionCard index=(i as u32) + 1 question=q />
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </Surface>
        </div>
    }
}

#[component]
fn QuestionCard(index: u32, question: EvaluationQuestionDto) -> impl IntoView {
    let references = question.references;
    let ref_count = references.len();
    let question_text = question.question;

    view! {
        <div class="surface-raised rounded p-4 space-y-3">
            <div class="flex items-start gap-3">
                <span class="font-mono text-xs muted shrink-0 pt-0.5">{format!("Q{index:02}")}</span>
                <p class="text-text leading-relaxed">{question_text}</p>
            </div>
            <div class="pl-8 space-y-2">
                <div class="eyebrow">{format!("References ({ref_count})")}</div>
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
        </div>
    }
}

#[component]
fn ReferenceCard(index: u32, reference: EvaluationReferenceDto) -> impl IntoView {
    let span = format!("[{}–{}]", reference.char_start, reference.char_end);
    let content = reference.content;
    view! {
        <div class="border-l-2 border-[var(--color-border)] pl-3 py-1">
            <div class="flex items-center gap-2 mb-1">
                <span class="font-mono text-xs muted">{format!("ref {index}")}</span>
                <span class="font-mono text-xs faint">{span}</span>
            </div>
            <p class="text-sm text-text whitespace-pre-wrap">{content}</p>
        </div>
    }
}
