use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use uuid::Uuid;

use crate::server_functions::configuration::get_configuration;
use crate::server_functions::evaluation::{
    get_datasets_for_document, start_generate_synthetic_dataset,
};
use crate::shared::contracts::{
    aggregate_type, AxisWeightDto, EmbeddingModelDto, EvaluationDatasetSummaryDto,
    GenerationModelDto,
};

fn default_weight_matrix() -> Vec<AxisWeightDto> {
    let entries: &[(&str, &str, u32)] = &[
        ("recall", "observation", 25),
        ("comprehend", "observation", 15),
        ("comprehend", "thread", 10),
        ("analyse", "thread", 10),
        ("apply", "thread", 10),
        ("analyse", "insight", 5),
        ("synthesise", "insight", 10),
        ("evaluate", "insight", 5),
        ("adversarial", "observation", 10),
    ];
    entries
        .iter()
        .map(|(op, ev, w)| AxisWeightDto {
            operation: (*op).to_string(),
            evidence: (*ev).to_string(),
            weight: *w,
        })
        .collect()
}
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, Help, Status, StatusPill, Surface};
use crate::ui::pages::evaluate::state::EvaluateSelection;

#[component]
pub fn DatasetStep(
    document_id: Uuid,
    selection: EvaluateSelection,
    on_advance: Callback<()>,
) -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_DATASET]));
    let datasets = Resource::new(
        move || (document_id, invalidator.get()),
        move |(id, _)| async move { get_datasets_for_document(id).await.unwrap_or_default() },
    );

    let catalog_invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::EMBEDDING_MODEL_CATALOG,
            aggregate_type::GENERATION_MODEL_CATALOG,
        ])
    });
    let catalog = Resource::new(
        move || catalog_invalidator.get(),
        |_| async move { get_configuration().await.ok() },
    );

    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (dataset_label, set_dataset_label) = signal("synthetic-default".to_string());
    let (question_count, set_question_count) = signal(30u32);
    // excerpt threshold removed — references come from comprehension map seeds.
    let (duplicate_threshold, set_duplicate_threshold) = signal(820u32);
    let (generation_model_id, set_generation_model_id) = signal::<Option<Uuid>>(None);
    let (embedding_model_id, set_embedding_model_id) = signal::<Option<Uuid>>(None);

    Effect::new(move |_| {
        if selection.dataset_id.get_untracked().is_none() {
            if let Some(list) = datasets.get() {
                if let Some(first) = list.first() {
                    selection.dataset_id.set(Some(first.dataset_id));
                }
            }
        }
    });

    Effect::new(move |_| {
        let Some(Some(cfg)) = catalog.get() else {
            return;
        };
        if generation_model_id.get_untracked().is_none() {
            if let Some(m) = cfg.generation_models.first() {
                set_generation_model_id.set(Some(m.generation_model_id));
            }
        }
        if embedding_model_id.get_untracked().is_none() {
            let default = cfg
                .index_profiles
                .iter()
                .find(|p| p.is_default)
                .map(|p| p.embedding_model_id)
                .or_else(|| cfg.embedding_models.first().map(|m| m.embedding_model_id));
            if let Some(id) = default {
                set_embedding_model_id.set(Some(id));
            }
        }
    });

    let on_generate = move |_| {
        let Some(gen_id) = generation_model_id.get() else {
            set_error.set(Some(
                "Pick a generation model before generating a dataset.".to_string(),
            ));
            return;
        };
        let Some(emb_id) = embedding_model_id.get() else {
            set_error.set(Some(
                "Pick an embedding model before generating a dataset.".to_string(),
            ));
            return;
        };
        let label = dataset_label.get();
        if label.trim().is_empty() {
            set_error.set(Some("Label cannot be empty.".to_string()));
            return;
        }
        let target = question_count.get();
        let duplicate = duplicate_threshold.get();
        set_busy.set(true);
        set_error.set(None);
        let weight_matrix = default_weight_matrix();
        spawn_local(async move {
            match start_generate_synthetic_dataset(
                document_id,
                gen_id,
                emb_id,
                label,
                target,
                duplicate,
                weight_matrix,
            )
            .await
            {
                Ok(_) => set_busy.set(false),
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    view! {
        <div class="space-y-6">
            <Surface
                title="Dataset".to_string()
                actions=Box::new(move || view! {
                    <Help title="What is an evaluation dataset?".to_string()>
                        <p>
                            "An evaluation dataset is a synthetic Q&A set generated from this document. Each question has a reference excerpt from the source; runs score how well a chunking + retrieval configuration finds the right context."
                        </p>
                        <p class="mt-3">
                            "Reuse a dataset across many runs to compare configurations on the same questions, or generate a new one when the document changes or you want different question characteristics."
                        </p>
                    </Help>
                }.into_any())
            >
                <Transition fallback=|| view! { <p class="muted text-sm">"Loading datasets…"</p> }>
                    {move || {
                        let list = datasets.get().unwrap_or_default();
                        if list.is_empty() {
                            return view! {
                                <EmptyState
                                    title="No datasets yet"
                                    body="Generate your first dataset below to start evaluating.".to_string()
                                />
                            }.into_any();
                        }
                        view! {
                            <div class="space-y-2">
                                {list.into_iter().map(|d| {
                                    let did = d.dataset_id;
                                    let is_active = move || selection.dataset_id.get() == Some(did);
                                    view! {
                                        <DatasetRow
                                            dataset=d
                                            is_active=is_active
                                            on_select=move || selection.dataset_id.set(Some(did))
                                        />
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </Transition>
            </Surface>

            <Surface title="Generate a new dataset".to_string()>
                <Transition fallback=|| view! { <p class="muted text-sm">"Loading models…"</p> }>
                    {move || {
                        catalog.get().map(|cfg| match cfg {
                            None => view! { <p class="muted text-sm">"Failed to load models."</p> }.into_any(),
                            Some(cfg) => view! {
                                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                                    <GenerationModelSelect
                                        models=cfg.generation_models
                                        value=generation_model_id
                                        set_value=set_generation_model_id
                                    />
                                    <EmbeddingModelSelect
                                        models=cfg.embedding_models
                                        value=embedding_model_id
                                        set_value=set_embedding_model_id
                                    />
                                </div>
                            }.into_any(),
                        })
                    }}
                </Transition>
                <div class="mt-4 space-y-4">
                    <label class="block space-y-1">
                        <span class="eyebrow">"Label"</span>
                        <input
                            class="input"
                            placeholder="dataset label"
                            prop:value=move || dataset_label.get()
                            on:input=move |ev| set_dataset_label.set(event_target_value(&ev))
                        />
                    </label>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                        <NumOption
                            label="Question count".to_string()
                            hint="Target questions per dataset".to_string()
                            value=question_count
                            set_value=set_question_count
                            min=1
                            max=10_000
                        />
                        <NumOption
                            label="Duplicate threshold (milli)".to_string()
                            hint="0–1000 · filters near-duplicate questions".to_string()
                            value=duplicate_threshold
                            set_value=set_duplicate_threshold
                            min=0
                            max=1000
                        />
                    </div>
                    <div class="flex items-center justify-between pt-3 border-t border-[var(--color-border)]">
                        {move || error.get().map(|e| view! {
                            <div class="log-line-error text-sm">{e}</div>
                        })}
                        <div class="ml-auto">
                            <button
                                type="button"
                                class="btn btn-primary"
                                disabled=move || busy.get()
                                on:click=on_generate
                            >
                                {move || if busy.get() { "Generating…" } else { "Generate dataset" }}
                            </button>
                        </div>
                    </div>
                </div>
            </Surface>

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Next"</span>
                    <span class="step-advance-eyebrow-label">"Configure and launch a run"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=move || selection.dataset_id.get().is_none()
                    on:click=move |_| on_advance.run(())
                >
                    {move || if selection.dataset_id.get().is_some() {
                        "Continue to run →"
                    } else {
                        "Select a dataset to continue"
                    }}
                </button>
            </div>
        </div>
    }
}

#[component]
fn DatasetRow<F>(
    dataset: EvaluationDatasetSummaryDto,
    is_active: F,
    on_select: impl Fn() + Send + Sync + 'static,
) -> impl IntoView
where
    F: Fn() -> bool + Send + Sync + 'static,
{
    let on_select_stored = StoredValue::new(on_select);
    let (kind, label) = eval_status(&dataset.status);
    let q_count = dataset.question_count;
    let dataset_label = dataset.label;
    let dataset_id = dataset.dataset_id;

    view! {
        <div
            class=move || format!(
                "w-full flex items-center justify-between gap-3 px-3 py-2 rounded border transition-colors {}",
                if is_active() {
                    "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                } else {
                    "border-[var(--color-border)] hover:border-[var(--color-border-strong)]"
                }
            )
        >
            <button
                type="button"
                class="flex-1 text-left text-text"
                on:click=move |_| on_select_stored.with_value(|f| f())
            >
                {dataset_label}
            </button>
            <div class="flex items-center gap-3">
                <span class="text-xs muted">{format!("{q_count} questions")}</span>
                <StatusPill label=label.to_string() kind=kind />
                <A
                    href=format!("/evaluations/datasets/{dataset_id}")
                    attr:class="text-xs muted hover:text-text underline-offset-2 hover:underline"
                >
                    "View →"
                </A>
            </div>
        </div>
    }
}

#[component]
fn GenerationModelSelect(
    models: Vec<GenerationModelDto>,
    value: ReadSignal<Option<Uuid>>,
    set_value: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 text-sm">
            <span class="eyebrow">"Generation model"</span>
            <select
                class="input"
                on:change=move |ev| {
                    set_value.set(Uuid::parse_str(&event_target_value(&ev)).ok());
                }
            >
                {models.into_iter().map(|m| {
                    let id = m.generation_model_id;
                    let label = format!("{} · {}", m.model, m.kind.as_str());
                    let selected = move || value.get() == Some(id);
                    view! { <option value=id.to_string() selected=selected>{label}</option> }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn EmbeddingModelSelect(
    models: Vec<EmbeddingModelDto>,
    value: ReadSignal<Option<Uuid>>,
    set_value: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 text-sm">
            <span class="eyebrow">"Embedding model"</span>
            <select
                class="input"
                on:change=move |ev| {
                    set_value.set(Uuid::parse_str(&event_target_value(&ev)).ok());
                }
            >
                {models.into_iter().map(|m| {
                    let id = m.embedding_model_id;
                    let label = format!("{} · {}d · {}", m.model, m.dimensions, m.kind.as_str());
                    let selected = move || value.get() == Some(id);
                    view! { <option value=id.to_string() selected=selected>{label}</option> }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn NumOption(
    label: String,
    hint: String,
    value: ReadSignal<u32>,
    set_value: WriteSignal<u32>,
    #[prop(default = 0)] min: u32,
    #[prop(default = u32::MAX)] max: u32,
) -> impl IntoView {
    view! {
        <label class="block space-y-1">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                type="number"
                min=min
                max=max
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let v: u32 = event_target_value(&e).parse().unwrap_or(min);
                    set_value.set(v.clamp(min, max));
                }
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

fn eval_status(status: &str) -> (Status, &'static str) {
    match status {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "running" => (Status::Pending, "Running"),
        "generating" => (Status::Pending, "Generating"),
        "pending" => (Status::Pending, "Pending"),
        "cancelled" => (Status::Cancel, "Cancelled"),
        _ => (Status::Neutral, "Unknown"),
    }
}
