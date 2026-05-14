use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{Dialog, EmptyState, PageHeader, Surface};
use crate::pages::configuration::commands::{
    run_embedding_model_command, run_generation_model_command, run_vector_index_command, short_uuid,
};
use crate::server_functions::configuration::get_configuration;
use crate::server_functions::settings::{load_settings, save_settings};
use crate::shared::{
    aggregate_type, AddEmbeddingModelDto, AddGenerationModelDto, AddVectorIndexDto,
    AiProviderKindDto, ConfigurationDto, EmbeddingModelCommandDto, EmbeddingModelDto,
    EvaluationGenerationBackend, GenerationModelCommandDto, GenerationModelDto,
    RemoveEmbeddingModelDto, RemoveGenerationModelDto, RemoveVectorIndexDto, SettingsDto,
    UpdateEmbeddingModelDto, UpdateGenerationModelDto, UpdateVectorIndexDto, VectorIndexCommandDto,
    VectorIndexDto, VectorStoreKindDto,
};

#[derive(Clone)]
enum CatalogCommand {
    Embedding(EmbeddingModelCommandDto),
    Generation(GenerationModelCommandDto),
    VectorIndex(VectorIndexCommandDto),
}

fn dispatch_catalog_command<F: FnOnce() + 'static>(
    command: CatalogCommand,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) {
    match command {
        CatalogCommand::Embedding(cmd) => run_embedding_model_command(
            cmd,
            success_message,
            set_busy,
            set_status,
            dialog_status,
            set_refresh,
            on_success,
        ),
        CatalogCommand::Generation(cmd) => run_generation_model_command(
            cmd,
            success_message,
            set_busy,
            set_status,
            dialog_status,
            set_refresh,
            on_success,
        ),
        CatalogCommand::VectorIndex(cmd) => run_vector_index_command(
            cmd,
            success_message,
            set_busy,
            set_status,
            dialog_status,
            set_refresh,
            on_success,
        ),
    }
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::EMBEDDING_MODEL_CATALOG,
            aggregate_type::GENERATION_MODEL_CATALOG,
            aggregate_type::VECTOR_INDEX_CATALOG,
        ])
    });
    let (refresh, set_refresh) = signal(0u32);

    let configuration = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_configuration().await.map_err(|e| e.to_string()) },
    );
    let settings = Resource::new(
        || (),
        |_| async move { load_settings().await.map_err(|e| e.to_string()) },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<(bool, String)>>(None);

    view! {
        <div>
            <PageHeader
                title="Settings"
                subtitle="Catalogue of models and indexes that pipelines compose. Plus defaults for the evaluation generator.".to_string()
            />

            <StatusBanner status=status />

            <Transition fallback=|| view! { <p class="muted">"Loading settings…"</p> }>
                {move || configuration.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error">{format!("Failed to load registry: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(cfg) => view! {
                        <Registry
                            config=cfg
                            busy=busy
                            set_busy=set_busy
                            set_status=set_status
                            set_refresh=set_refresh
                        />
                    }.into_any(),
                })}
            </Transition>

            <div class="mt-8">
                <Transition fallback=|| view! { <p class="muted">"Loading defaults…"</p> }>
                    {move || settings.get().map(|res| match res {
                        Err(e) => view! {
                            <Surface>
                                <div class="log-line-error">{format!("Failed to load settings: {e}")}</div>
                            </Surface>
                        }.into_any(),
                        Ok(s) => view! { <EvaluationDefaults initial=s /> }.into_any(),
                    })}
                </Transition>
            </div>
        </div>
    }
}

#[component]
fn StatusBanner(status: ReadSignal<Option<(bool, String)>>) -> impl IntoView {
    view! {
        {move || status.get().map(|(ok, msg)| {
            let cls = if ok {
                "surface mb-4 px-4 py-2"
            } else {
                "surface mb-4 px-4 py-2 log-line-error"
            };
            view! { <div class=cls>{msg}</div> }
        })}
    }
}

#[derive(Clone)]
enum RegistryForm {
    AddEmbeddingModel,
    EditEmbeddingModel(EmbeddingModelDto),
    AddGenerationModel,
    EditGenerationModel(GenerationModelDto),
    AddVectorIndex,
    EditVectorIndex(VectorIndexDto),
}

#[derive(Clone)]
enum DeleteTarget {
    EmbeddingModel(EmbeddingModelDto),
    GenerationModel(GenerationModelDto),
    VectorIndex(VectorIndexDto),
}

impl DeleteTarget {
    fn label(&self) -> String {
        match self {
            Self::EmbeddingModel(m) => format!("Embedding model · {}", m.model),
            Self::GenerationModel(m) => format!("Generation model · {}", m.model),
            Self::VectorIndex(i) => format!("Vector index · {}", i.name),
        }
    }
}

#[component]
fn Registry(
    config: ConfigurationDto,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let config = StoredValue::new(config);
    let (form, set_form) = signal::<Option<RegistryForm>>(None);
    let (delete_target, set_delete_target) = signal::<Option<DeleteTarget>>(None);

    let open_form = Callback::new(move |f: RegistryForm| set_form.set(Some(f)));
    let open_delete = Callback::new(move |t: DeleteTarget| set_delete_target.set(Some(t)));

    view! {
        <div class="space-y-6">
            <EmbeddingModelsSection config=config busy=busy open_form=open_form open_delete=open_delete />
            <GenerationModelsSection config=config busy=busy open_form=open_form open_delete=open_delete />
            <VectorIndexesSection config=config busy=busy open_form=open_form open_delete=open_delete />
        </div>

        <RegistryFormDialog
            form=form
            set_form=set_form
            busy=busy
            set_busy=set_busy
            set_status=set_status
            set_refresh=set_refresh
        />

        <RegistryDeleteDialog
            target=delete_target
            set_target=set_delete_target
            busy=busy
            set_busy=set_busy
            set_status=set_status
            set_refresh=set_refresh
        />
    }
}

#[component]
fn EmbeddingModelsSection(
    config: StoredValue<ConfigurationDto>,
    busy: ReadSignal<bool>,
    open_form: Callback<RegistryForm>,
    open_delete: Callback<DeleteTarget>,
) -> impl IntoView {
    view! {
        <Surface
            title="Embedding models".to_string()
            actions=Box::new(move || view! {
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=busy
                    on:click=move |_| open_form.run(RegistryForm::AddEmbeddingModel)
                >
                    "+ Add embedding model"
                </button>
            }.into_any())
        >
            {move || {
                let cfg = config.get_value();
                if cfg.embedding_models.is_empty() {
                    view! {
                        <EmptyState
                            title="No embedding models yet"
                            body="Register the embedding models you want pipelines to use.".to_string()
                        />
                    }.into_any()
                } else {
                    view! {
                        <div class="space-y-2">
                            {cfg.embedding_models.iter().map(|m| {
                                let edit_target = m.clone();
                                let delete_target = m.clone();
                                view! {
                                    <RegistryRow
                                        title=m.model.clone()
                                        subtitle=format!("{} · {}d · {}", m.kind.display_label(), m.dimensions, short_uuid(m.embedding_model_id))
                                        on_edit=move || open_form.run(RegistryForm::EditEmbeddingModel(edit_target.clone()))
                                        on_delete=move || open_delete.run(DeleteTarget::EmbeddingModel(delete_target.clone()))
                                        busy=busy
                                    />
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </Surface>
    }
}

#[component]
fn GenerationModelsSection(
    config: StoredValue<ConfigurationDto>,
    busy: ReadSignal<bool>,
    open_form: Callback<RegistryForm>,
    open_delete: Callback<DeleteTarget>,
) -> impl IntoView {
    view! {
        <Surface
            title="Generation models".to_string()
            actions=Box::new(move || view! {
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=busy
                    on:click=move |_| open_form.run(RegistryForm::AddGenerationModel)
                >
                    "+ Add generation model"
                </button>
            }.into_any())
        >
            {move || {
                let cfg = config.get_value();
                if cfg.generation_models.is_empty() {
                    view! {
                        <EmptyState
                            title="No generation models yet"
                            body="Generation models power LLM-driven chunking and synthetic dataset generation.".to_string()
                        />
                    }.into_any()
                } else {
                    view! {
                        <div class="space-y-2">
                            {cfg.generation_models.iter().map(|m| {
                                let edit_target = m.clone();
                                let delete_target = m.clone();
                                view! {
                                    <RegistryRow
                                        title=m.model.clone()
                                        subtitle=format!("{} · {}", m.kind.display_label(), short_uuid(m.generation_model_id))
                                        on_edit=move || open_form.run(RegistryForm::EditGenerationModel(edit_target.clone()))
                                        on_delete=move || open_delete.run(DeleteTarget::GenerationModel(delete_target.clone()))
                                        busy=busy
                                    />
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </Surface>
    }
}

#[component]
fn VectorIndexesSection(
    config: StoredValue<ConfigurationDto>,
    busy: ReadSignal<bool>,
    open_form: Callback<RegistryForm>,
    open_delete: Callback<DeleteTarget>,
) -> impl IntoView {
    view! {
        <Surface
            title="Vector indexes".to_string()
            actions=Box::new(move || view! {
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=busy
                    on:click=move |_| open_form.run(RegistryForm::AddVectorIndex)
                >
                    "+ Add vector index"
                </button>
            }.into_any())
        >
            {move || {
                let cfg = config.get_value();
                if cfg.vector_indexes.is_empty() {
                    view! {
                        <EmptyState
                            title="No vector indexes yet"
                            body="Register the vector indexes embeddings should write to.".to_string()
                        />
                    }.into_any()
                } else {
                    view! {
                        <div class="space-y-2">
                            {cfg.vector_indexes.iter().map(|i| {
                                let edit_target = i.clone();
                                let delete_target = i.clone();
                                view! {
                                    <RegistryRow
                                        title=i.name.clone()
                                        subtitle=format!("{} · {}d · {}", i.kind.display_label(), i.dimensions, short_uuid(i.index_id))
                                        on_edit=move || open_form.run(RegistryForm::EditVectorIndex(edit_target.clone()))
                                        on_delete=move || open_delete.run(DeleteTarget::VectorIndex(delete_target.clone()))
                                        busy=busy
                                    />
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </Surface>
    }
}

#[component]
fn RegistryRow(
    title: String,
    subtitle: String,
    on_edit: impl Fn() + Send + Sync + 'static,
    on_delete: impl Fn() + Send + Sync + 'static,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let on_edit = StoredValue::new(on_edit);
    let on_delete = StoredValue::new(on_delete);
    view! {
        <div class="surface-raised rounded p-3 flex items-center justify-between gap-3">
            <div class="min-w-0">
                <div class="text-text font-medium truncate">{title}</div>
                <div class="text-xs muted truncate">{subtitle}</div>
            </div>
            <div class="flex gap-2 shrink-0">
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_edit.with_value(|f| f())
                >"Edit"</button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_delete.with_value(|f| f())
                >"Delete"</button>
            </div>
        </div>
    }
}

#[component]
fn RegistryFormDialog(
    form: ReadSignal<Option<RegistryForm>>,
    set_form: WriteSignal<Option<RegistryForm>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    let (name, set_name) = signal(String::new());
    let (ai_kind, set_ai_kind) = signal(AiProviderKindDto::Cloudflare);
    let (vector_kind, set_vector_kind) = signal(VectorStoreKindDto::CloudflareVectorize);
    let (model_id, set_model_id) = signal(String::new());
    let (dims, set_dims) = signal(1024u32);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form.get() {
            None => {}
            Some(RegistryForm::AddEmbeddingModel) => {
                set_ai_kind.set(AiProviderKindDto::Cloudflare);
                set_model_id.set(String::new());
                set_dims.set(1024);
            }
            Some(RegistryForm::EditEmbeddingModel(m)) => {
                set_ai_kind.set(m.kind);
                set_model_id.set(m.model);
                set_dims.set(m.dimensions);
            }
            Some(RegistryForm::AddGenerationModel) => {
                set_ai_kind.set(AiProviderKindDto::Cloudflare);
                set_model_id.set(String::new());
            }
            Some(RegistryForm::EditGenerationModel(m)) => {
                set_ai_kind.set(m.kind);
                set_model_id.set(m.model);
            }
            Some(RegistryForm::AddVectorIndex) => {
                set_vector_kind.set(VectorStoreKindDto::CloudflareVectorize);
                set_name.set(String::new());
                set_dims.set(1024);
            }
            Some(RegistryForm::EditVectorIndex(i)) => {
                set_vector_kind.set(i.kind);
                set_name.set(i.name);
                set_dims.set(i.dimensions);
            }
        }
    });

    let close = Callback::new(move |_| {
        set_form.set(None);
        set_dialog_error.set(None);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(active) = form.get_untracked() else {
            return;
        };
        let command = match build_command(
            active,
            name.get_untracked(),
            ai_kind.get_untracked(),
            vector_kind.get_untracked(),
            model_id.get_untracked(),
            dims.get_untracked(),
        ) {
            Ok(c) => c,
            Err(msg) => {
                set_dialog_error.set(Some(msg));
                return;
            }
        };
        dispatch_catalog_command(
            command,
            "Saved",
            set_busy,
            set_status,
            Some(set_dialog_error),
            set_refresh,
            move || set_form.set(None),
        );
    };

    view! {
        <Dialog
            open=Signal::derive(move || form.get().is_some())
            title=Signal::derive(move || form_title(form.get())).get()
            subtitle=Signal::derive(move || form_subtitle(form.get())).get()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|m| view! {
                    <div class="log-line-error text-sm">{m}</div>
                })}

                {move || match form.get() {
                    None => ().into_any(),
                    Some(RegistryForm::AddEmbeddingModel) | Some(RegistryForm::EditEmbeddingModel(_)) => view! {
                        <AiKindSelect value=ai_kind set_value=set_ai_kind />
                        <LabelledInput
                            label="Model ID".to_string()
                            hint="Provider-specific model identifier (e.g. @cf/baai/bge-base-en-v1.5)".to_string()
                            value=model_id
                            set_value=set_model_id
                        />
                        <LabelledNum
                            label="Dimensions".to_string()
                            hint="Must match the target vector index".to_string()
                            value=dims
                            set_value=set_dims
                            min=1
                        />
                    }.into_any(),
                    Some(RegistryForm::AddGenerationModel) | Some(RegistryForm::EditGenerationModel(_)) => view! {
                        <AiKindSelect value=ai_kind set_value=set_ai_kind />
                        <LabelledInput
                            label="Model ID".to_string()
                            hint="Chat/completion model identifier".to_string()
                            value=model_id
                            set_value=set_model_id
                        />
                    }.into_any(),
                    Some(RegistryForm::AddVectorIndex) | Some(RegistryForm::EditVectorIndex(_)) => view! {
                        <VectorKindSelect value=vector_kind set_value=set_vector_kind />
                        <LabelledInput
                            label="Index name".to_string()
                            hint="External vector store identifier".to_string()
                            value=name
                            set_value=set_name
                        />
                        <LabelledNum
                            label="Dimensions".to_string()
                            hint="Must match the embedding model output".to_string()
                            value=dims
                            set_value=set_dims
                            min=1
                        />
                    }.into_any(),
                }}

                <div class="flex justify-end gap-2 pt-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="submit" class="btn btn-primary" disabled=busy>
                        {move || if busy.get() { "Saving…" } else { "Save" }}
                    </button>
                </div>
            </form>
        </Dialog>
    }
}

#[component]
fn AiKindSelect(
    value: ReadSignal<AiProviderKindDto>,
    set_value: WriteSignal<AiProviderKindDto>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">"Provider"</span>
            <select
                class="input"
                on:change=move |e| {
                    let v = event_target_value(&e);
                    let kind = AiProviderKindDto::all()
                        .iter()
                        .copied()
                        .find(|k| k.as_str() == v)
                        .unwrap_or(AiProviderKindDto::Cloudflare);
                    set_value.set(kind);
                }
            >
                {AiProviderKindDto::all().iter().copied().map(|k| {
                    let key = k.as_str();
                    let label = k.display_label();
                    view! {
                        <option value=key selected=move || value.get() == k>{label}</option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn VectorKindSelect(
    value: ReadSignal<VectorStoreKindDto>,
    set_value: WriteSignal<VectorStoreKindDto>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">"Vector store"</span>
            <select
                class="input"
                on:change=move |e| {
                    let v = event_target_value(&e);
                    let kind = VectorStoreKindDto::all()
                        .iter()
                        .copied()
                        .find(|k| k.as_str() == v)
                        .unwrap_or(VectorStoreKindDto::CloudflareVectorize);
                    set_value.set(kind);
                }
            >
                {VectorStoreKindDto::all().iter().copied().map(|k| {
                    let key = k.as_str();
                    let label = k.display_label();
                    view! {
                        <option value=key selected=move || value.get() == k>{label}</option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn RegistryDeleteDialog(
    target: ReadSignal<Option<DeleteTarget>>,
    set_target: WriteSignal<Option<DeleteTarget>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(t) = target.get_untracked() else {
            return;
        };
        let command = match t {
            DeleteTarget::EmbeddingModel(m) => CatalogCommand::Embedding(
                EmbeddingModelCommandDto::RemoveEmbeddingModel(RemoveEmbeddingModelDto {
                    model_id: m.embedding_model_id,
                }),
            ),
            DeleteTarget::GenerationModel(m) => CatalogCommand::Generation(
                GenerationModelCommandDto::RemoveGenerationModel(RemoveGenerationModelDto {
                    model_id: m.generation_model_id,
                }),
            ),
            DeleteTarget::VectorIndex(i) => CatalogCommand::VectorIndex(
                VectorIndexCommandDto::RemoveVectorIndex(RemoveVectorIndexDto {
                    index_id: i.index_id,
                }),
            ),
        };
        dispatch_catalog_command(
            command,
            "Deleted",
            set_busy,
            set_status,
            None,
            set_refresh,
            move || set_target.set(None),
        );
    };

    view! {
        <Dialog
            open=Signal::derive(move || target.get().is_some())
            title="Confirm delete".to_string()
            subtitle="Downstream pipelines referencing this entry must be removed first.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised rounded p-3">
                    <span class="text-text">{move || target.get().map(|t| t.label()).unwrap_or_default()}</span>
                </div>
                <div class="flex justify-end gap-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="button" class="btn btn-primary" disabled=busy on:click=confirm>
                        {move || if busy.get() { "Deleting…" } else { "Delete" }}
                    </button>
                </div>
            </div>
        </Dialog>
    }
}

#[component]
fn EvaluationDefaults(initial: SettingsDto) -> impl IntoView {
    let (eval, set_eval) = signal(initial.evaluation.clone());
    let (status, set_status) = signal::<Option<(bool, String)>>(None);
    let (saving, set_saving) = signal(false);

    let initial_stored = StoredValue::new(initial);

    let on_save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let mut payload = initial_stored.get_value();
        payload.evaluation = eval.get();
        set_saving.set(true);
        set_status.set(None);
        spawn_local(async move {
            let result = save_settings(payload).await;
            set_saving.set(false);
            match result {
                Ok(()) => set_status.set(Some((true, "Saved".into()))),
                Err(e) => set_status.set(Some((false, format!("Save failed: {e}")))),
            }
        });
    };

    view! {
        <Surface
            title="Evaluation defaults".to_string()
            actions=Box::new(move || view! {
                <span class="text-xs faint">"Used by the synthetic dataset generator."</span>
            }.into_any())
        >
            <form on:submit=on_save class="space-y-4">
                <p class="muted text-sm">
                    "These tune the question-generation pipeline that builds evaluation datasets. \
                     They are intentionally separate from a pipeline's embedding/generation model selection."
                </p>

                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                    <LabelledSelectStatic
                        label="Generation backend".to_string()
                        value=Signal::derive(move || eval.get().generation_backend.as_str().to_string())
                        on_change=move |v: String| set_eval.update(|c| {
                            c.generation_backend = match v.as_str() {
                                "workers_ai" => EvaluationGenerationBackend::WorkersAi,
                                _ => EvaluationGenerationBackend::Ollama,
                            };
                        })
                        options=vec![
                            ("ollama".to_string(), "Ollama".to_string()),
                            ("workers_ai".to_string(), "Workers AI".to_string()),
                        ]
                    />
                    <LabelledInputDirect
                        label="Generation model".to_string()
                        hint="Chat model used to generate questions".to_string()
                        value=Signal::derive(move || eval.get().generation_model.clone())
                        on_change=move |v: String| set_eval.update(|c| c.generation_model = v)
                    />
                    <LabelledNumDirect
                        label="Question count".to_string()
                        hint="Target questions per document".to_string()
                        value=Signal::derive(move || eval.get().question_count)
                        on_change=move |v| set_eval.update(|c| c.question_count = v)
                        min=1
                    />
                    <LabelledNumDirect
                        label="Top-k".to_string()
                        hint="Default chunks retrieved per question".to_string()
                        value=Signal::derive(move || eval.get().top_k)
                        on_change=move |v| set_eval.update(|c| c.top_k = v)
                        min=1
                    />
                    <LabelledNumDirect
                        label="Min score (milli)".to_string()
                        hint="0–1000 cosine threshold".to_string()
                        value=Signal::derive(move || eval.get().min_score_milli)
                        on_change=move |v| set_eval.update(|c| c.min_score_milli = v.min(1000))
                        min=0
                    />
                    <LabelledNumDirect
                        label="Excerpt threshold (milli)".to_string()
                        hint="Filters weak query/reference pairs".to_string()
                        value=Signal::derive(move || eval.get().excerpt_similarity_threshold_milli)
                        on_change=move |v| set_eval.update(|c| c.excerpt_similarity_threshold_milli = v.min(1000))
                        min=0
                    />
                    <LabelledNumDirect
                        label="Duplicate threshold (milli)".to_string()
                        hint="Filters near-duplicate questions".to_string()
                        value=Signal::derive(move || eval.get().duplicate_similarity_threshold_milli)
                        on_change=move |v| set_eval.update(|c| c.duplicate_similarity_threshold_milli = v.min(1000))
                        min=0
                    />
                </div>

                <div class="flex items-center gap-3 pt-2">
                    <button type="submit" class="btn btn-primary" disabled=saving>
                        {move || if saving.get() { "Saving…" } else { "Save defaults" }}
                    </button>
                    {move || status.get().map(|(ok, msg)| {
                        let cls = if ok { "text-sm" } else { "text-sm log-line-error" };
                        view! { <span class=cls>{msg}</span> }
                    })}
                </div>
            </form>
        </Surface>
    }
}

#[component]
fn LabelledInput(
    label: String,
    hint: String,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                prop:value=move || value.get()
                on:input=move |e| set_value.set(event_target_value(&e))
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

#[component]
fn LabelledNum(
    label: String,
    hint: String,
    value: ReadSignal<u32>,
    set_value: WriteSignal<u32>,
    #[prop(default = 0)] min: u32,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                type="number"
                min=min
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let v: u32 = event_target_value(&e).parse().unwrap_or(min);
                    set_value.set(v.max(min));
                }
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

#[component]
fn LabelledSelectStatic(
    label: String,
    value: Signal<String>,
    on_change: impl Fn(String) + Send + Sync + 'static,
    options: Vec<(String, String)>,
) -> impl IntoView {
    let on_change = StoredValue::new(on_change);
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <select
                class="input"
                on:change=move |e| on_change.with_value(|f| f(event_target_value(&e)))
            >
                {options.into_iter().map(|(v, lab)| {
                    let v_clone = v.clone();
                    view! {
                        <option value=v.clone() selected=move || value.get() == v_clone>
                            {lab}
                        </option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn LabelledInputDirect(
    label: String,
    hint: String,
    value: Signal<String>,
    on_change: impl Fn(String) + Send + Sync + 'static,
) -> impl IntoView {
    let on_change = StoredValue::new(on_change);
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                prop:value=move || value.get()
                on:input=move |e| on_change.with_value(|f| f(event_target_value(&e)))
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

#[component]
fn LabelledNumDirect(
    label: String,
    hint: String,
    value: Signal<u32>,
    on_change: impl Fn(u32) + Send + Sync + 'static,
    #[prop(default = 0)] min: u32,
) -> impl IntoView {
    let on_change = StoredValue::new(on_change);
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                type="number"
                min=min
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let v: u32 = event_target_value(&e).parse().unwrap_or(min);
                    on_change.with_value(|f| f(v.max(min)));
                }
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

fn build_command(
    form: RegistryForm,
    name: String,
    ai_kind: AiProviderKindDto,
    vector_kind: VectorStoreKindDto,
    model_id: String,
    dims: u32,
) -> Result<CatalogCommand, String> {
    let name = name.trim().to_string();
    let model_id = model_id.trim().to_string();
    match form {
        RegistryForm::AddEmbeddingModel => {
            if model_id.is_empty() {
                return Err("Model id is required.".into());
            }
            Ok(CatalogCommand::Embedding(
                EmbeddingModelCommandDto::AddEmbeddingModel(AddEmbeddingModelDto {
                    kind: ai_kind,
                    model: model_id,
                    dimensions: dims,
                }),
            ))
        }
        RegistryForm::EditEmbeddingModel(m) => {
            if model_id.is_empty() {
                return Err("Model id is required.".into());
            }
            Ok(CatalogCommand::Embedding(
                EmbeddingModelCommandDto::UpdateEmbeddingModel(UpdateEmbeddingModelDto {
                    model_id: m.embedding_model_id,
                    kind: ai_kind,
                    model: model_id,
                    dimensions: dims,
                }),
            ))
        }
        RegistryForm::AddGenerationModel => {
            if model_id.is_empty() {
                return Err("Model id is required.".into());
            }
            Ok(CatalogCommand::Generation(
                GenerationModelCommandDto::AddGenerationModel(AddGenerationModelDto {
                    kind: ai_kind,
                    model: model_id,
                }),
            ))
        }
        RegistryForm::EditGenerationModel(m) => {
            if model_id.is_empty() {
                return Err("Model id is required.".into());
            }
            Ok(CatalogCommand::Generation(
                GenerationModelCommandDto::UpdateGenerationModel(UpdateGenerationModelDto {
                    model_id: m.generation_model_id,
                    kind: ai_kind,
                    model: model_id,
                }),
            ))
        }
        RegistryForm::AddVectorIndex => {
            if name.is_empty() {
                return Err("Index name is required.".into());
            }
            Ok(CatalogCommand::VectorIndex(
                VectorIndexCommandDto::AddVectorIndex(AddVectorIndexDto {
                    kind: vector_kind,
                    name,
                    dimensions: dims,
                }),
            ))
        }
        RegistryForm::EditVectorIndex(i) => {
            if name.is_empty() {
                return Err("Index name is required.".into());
            }
            Ok(CatalogCommand::VectorIndex(
                VectorIndexCommandDto::UpdateVectorIndex(UpdateVectorIndexDto {
                    index_id: i.index_id,
                    kind: vector_kind,
                    name,
                    dimensions: dims,
                }),
            ))
        }
    }
}

fn form_title(form: Option<RegistryForm>) -> String {
    match form {
        None => String::new(),
        Some(RegistryForm::AddEmbeddingModel) => "Add embedding model".into(),
        Some(RegistryForm::EditEmbeddingModel(_)) => "Edit embedding model".into(),
        Some(RegistryForm::AddGenerationModel) => "Add generation model".into(),
        Some(RegistryForm::EditGenerationModel(_)) => "Edit generation model".into(),
        Some(RegistryForm::AddVectorIndex) => "Add vector index".into(),
        Some(RegistryForm::EditVectorIndex(_)) => "Edit vector index".into(),
    }
}

fn form_subtitle(form: Option<RegistryForm>) -> String {
    match form {
        None => String::new(),
        Some(RegistryForm::AddEmbeddingModel) | Some(RegistryForm::EditEmbeddingModel(_)) => {
            "Dimensions must match the target vector index.".into()
        }
        Some(RegistryForm::AddGenerationModel) | Some(RegistryForm::EditGenerationModel(_)) => {
            "Used by LLM-driven chunking and synthetic dataset generation.".into()
        }
        Some(RegistryForm::AddVectorIndex) | Some(RegistryForm::EditVectorIndex(_)) => {
            "Dimensions must match the embedding model that writes into it.".into()
        }
    }
}
