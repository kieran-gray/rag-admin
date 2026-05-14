use leptos::prelude::*;
use leptos_router::components::A;
use uuid::Uuid;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{Dialog, EmptyState, PageHeader, Surface};
use crate::pages::configuration::commands::{
    parse_uuid_or_none, run_pipeline_configuration_command,
};
use crate::server_functions::configuration::{get_configuration, get_pipeline_configurations};
use crate::shared::{
    aggregate_type, ConfigurationDto, CreatePipelineConfigurationDto,
    DeletePipelineConfigurationDto, PipelineConfigurationCommandDto, PipelineConfigurationDto,
    UpdatePipelineConfigurationDto,
};

#[derive(Clone)]
enum FormMode {
    Add,
    Edit(PipelineConfigurationDto),
}

#[component]
pub fn PipelinesPage() -> impl IntoView {
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
    let pipelines = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move {
            get_pipeline_configurations()
                .await
                .map_err(|e| e.to_string())
        },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<(bool, String)>>(None);
    let (form_mode, set_form_mode) = signal::<Option<FormMode>>(None);
    let (delete_target, set_delete_target) = signal::<Option<PipelineConfigurationDto>>(None);

    view! {
        <div>
            <PageHeader
                title="Pipelines"
                subtitle="Named compositions of an embedding model, generation model, and vector index.".to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=move |_| set_form_mode.set(Some(FormMode::Add))
                    >
                        "+ New pipeline"
                    </button>
                }.into_any())
            />

            <StatusBanner status=status />

            <Transition fallback=|| view! { <p class="muted">"Loading pipelines…"</p> }>
                {move || {
                    let (cfg, list) = match (configuration.get(), pipelines.get()) {
                        (Some(Ok(c)), Some(Ok(l))) => (c, l),
                        (Some(Err(e)), _) | (_, Some(Err(e))) => {
                            return view! {
                                <Surface>
                                    <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                                </Surface>
                            }.into_any();
                        }
                        _ => return ().into_any(),
                    };

                    view! {
                        <PipelineList
                            pipelines=list
                            on_edit=Callback::new(move |pc: PipelineConfigurationDto| set_form_mode.set(Some(FormMode::Edit(pc))))
                            on_delete=Callback::new(move |pc: PipelineConfigurationDto| set_delete_target.set(Some(pc)))
                            registry_hint=cfg.embedding_models.is_empty()
                                || cfg.generation_models.is_empty()
                                || cfg.vector_indexes.is_empty()
                            busy=busy
                        />
                    }.into_any()
                }}
            </Transition>


            <Transition fallback=|| ()>
                {move || configuration.get().map(|res| match res {
                    Ok(cfg) => view! {
                        <PipelineFormDialog
                            config=cfg
                            form_mode=form_mode
                            set_form_mode=set_form_mode
                            busy=busy
                            set_busy=set_busy
                            set_status=set_status
                            set_refresh=set_refresh
                        />
                    }.into_any(),
                    Err(_) => ().into_any(),
                })}
            </Transition>


            <DeleteConfirmDialog
                target=delete_target
                set_target=set_delete_target
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />
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

#[component]
fn PipelineList(
    pipelines: Vec<PipelineConfigurationDto>,
    on_edit: Callback<PipelineConfigurationDto>,
    on_delete: Callback<PipelineConfigurationDto>,
    registry_hint: bool,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    if pipelines.is_empty() {
        return view! {
            <Surface>
                <EmptyState
                    title="No pipelines yet"
                    body=if registry_hint {
                        "Add at least one embedding model, generation model, and vector index in Settings, then come back to compose them into a pipeline.".to_string()
                    } else {
                        "Pipelines compose an embedding model, generation model, and vector index for a named environment.".to_string()
                    }
                    action=Box::new(|| view! {
                        <A href="/settings" attr:class="btn">"Open Settings"</A>
                    }.into_any())
                />
            </Surface>
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            {pipelines.into_iter().map(|pc| view! {
                <PipelineCard
                    pc=pc
                    on_edit=on_edit
                    on_delete=on_delete
                    busy=busy
                />
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn PipelineCard(
    pc: PipelineConfigurationDto,
    on_edit: Callback<PipelineConfigurationDto>,
    on_delete: Callback<PipelineConfigurationDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let pc_clone_edit = pc.clone();
    let pc_clone_delete = pc.clone();
    let name = pc.name.clone();
    let embedding = pc
        .embedding_model_name
        .clone()
        .unwrap_or_else(|| short_uuid(pc.embedding_model_id));
    let generation = pc
        .generation_model_name
        .clone()
        .unwrap_or_else(|| short_uuid(pc.generation_model_id));
    let index = pc
        .vector_index_name
        .clone()
        .unwrap_or_else(|| short_uuid(pc.vector_index_id));

    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <h3 class="section-title">{name}</h3>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{format!("embed · {embedding}")}</span>
                    <span class="pill pill-neutral">{format!("gen · {generation}")}</span>
                    <span class="pill pill-neutral">{format!("index · {index}")}</span>
                </div>
            </div>
            <div class="flex gap-2 shrink-0">
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_edit.run(pc_clone_edit.clone())
                >
                    "Edit"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_delete.run(pc_clone_delete.clone())
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}

#[component]
fn PipelineFormDialog(
    config: ConfigurationDto,
    form_mode: ReadSignal<Option<FormMode>>,
    set_form_mode: WriteSignal<Option<FormMode>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let config = StoredValue::new(config);
    let (name, set_name) = signal(String::new());
    let (embedding_id, set_embedding_id) = signal::<Option<Uuid>>(None);
    let (generation_id, set_generation_id) = signal::<Option<Uuid>>(None);
    let (vector_index_id, set_vector_index_id) = signal::<Option<Uuid>>(None);
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form_mode.get() {
            None => {}
            Some(FormMode::Add) => {
                set_name.set(String::new());
                set_embedding_id.set(
                    config.with_value(|c| c.embedding_models.first().map(|m| m.embedding_model_id)),
                );
                set_generation_id
                    .set(config.with_value(|c| {
                        c.generation_models.first().map(|m| m.generation_model_id)
                    }));
                set_vector_index_id
                    .set(config.with_value(|c| c.vector_indexes.first().map(|i| i.index_id)));
            }
            Some(FormMode::Edit(pc)) => {
                set_name.set(pc.name);
                set_embedding_id.set(Some(pc.embedding_model_id));
                set_generation_id.set(Some(pc.generation_model_id));
                set_vector_index_id.set(Some(pc.vector_index_id));
            }
        }
    });

    let close = Callback::new(move |_| {
        set_form_mode.set(None);
        set_dialog_error.set(None);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (Some(emb), Some(gen), Some(idx)) = (
            embedding_id.get(),
            generation_id.get(),
            vector_index_id.get(),
        ) else {
            set_dialog_error.set(Some(
                "Pick an embedding model, generation model, and vector index.".into(),
            ));
            return;
        };
        let name_val = name.get().trim().to_string();
        if name_val.is_empty() {
            set_dialog_error.set(Some("Pipeline name is required.".into()));
            return;
        }
        let command = match form_mode.get() {
            Some(FormMode::Add) => PipelineConfigurationCommandDto::CreatePipelineConfiguration(
                CreatePipelineConfigurationDto {
                    name: name_val,
                    embedding_model_id: emb,
                    generation_model_id: gen,
                    vector_index_id: idx,
                },
            ),
            Some(FormMode::Edit(pc)) => {
                PipelineConfigurationCommandDto::UpdatePipelineConfiguration(
                    UpdatePipelineConfigurationDto {
                        pipeline_configuration_id: pc.pipeline_configuration_id,
                        name: name_val,
                        embedding_model_id: emb,
                        generation_model_id: gen,
                        vector_index_id: idx,
                    },
                )
            }
            None => return,
        };
        run_pipeline_configuration_command(
            command,
            "Pipeline saved",
            set_busy,
            set_status,
            Some(set_dialog_error),
            set_refresh,
            move || set_form_mode.set(None),
        );
    };

    view! {
        <Dialog
            open=Signal::derive(move || form_mode.get().is_some())
            title=Signal::derive(move || match form_mode.get() {
                Some(FormMode::Edit(_)) => "Edit pipeline".to_string(),
                _ => "New pipeline".to_string(),
            }).get()
            subtitle="Pipeline dimensions must match between the embedding model and the vector index.".to_string()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|msg| view! {
                    <div class="log-line-error text-sm">{msg}</div>
                })}

                <LabelledInput
                    label="Name".to_string()
                    hint="e.g. production-cf, local-ollama".to_string()
                    value=name
                    set_value=set_name
                />

                <LabelledSelect
                    label="Embedding model".to_string()
                    placeholder="— select embedding model —".to_string()
                    value=embedding_id
                    set_value=set_embedding_id
                    options=Memo::new(move |_| config.with_value(|c| {
                        c.embedding_models
                            .iter()
                            .map(|m| (
                                m.embedding_model_id,
                                format!("{} · {} · {}d", m.kind.display_label(), m.model, m.dimensions),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <LabelledSelect
                    label="Generation model".to_string()
                    placeholder="— select generation model —".to_string()
                    value=generation_id
                    set_value=set_generation_id
                    options=Memo::new(move |_| config.with_value(|c| {
                        c.generation_models
                            .iter()
                            .map(|m| (
                                m.generation_model_id,
                                format!("{} · {}", m.kind.display_label(), m.model),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <LabelledSelect
                    label="Vector index".to_string()
                    placeholder="— select vector index —".to_string()
                    value=vector_index_id
                    set_value=set_vector_index_id
                    options=Memo::new(move |_| config.with_value(|c| {
                        c.vector_indexes
                            .iter()
                            .map(|i| (
                                i.index_id,
                                format!("{} · {} · {}d", i.kind.display_label(), i.name, i.dimensions),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <div class="flex justify-end gap-2 pt-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="submit" class="btn btn-primary" disabled=busy>
                        {move || if busy.get() { "Saving…" } else { "Save pipeline" }}
                    </button>
                </div>
            </form>
        </Dialog>
    }
}

#[component]
fn DeleteConfirmDialog(
    target: ReadSignal<Option<PipelineConfigurationDto>>,
    set_target: WriteSignal<Option<PipelineConfigurationDto>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(pc) = target.get_untracked() else {
            return;
        };
        run_pipeline_configuration_command(
            PipelineConfigurationCommandDto::DeletePipelineConfiguration(
                DeletePipelineConfigurationDto {
                    pipeline_configuration_id: pc.pipeline_configuration_id,
                },
            ),
            "Pipeline deleted",
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
            title="Delete pipeline".to_string()
            subtitle="The registry entries this pipeline references are not affected.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised p-3 rounded">
                    <span class="muted text-sm">"Pipeline"</span>
                    <div class="text-text">{move || target.get().map(|pc| pc.name).unwrap_or_default()}</div>
                </div>
                <div class="flex justify-end gap-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="button" class="btn btn-primary" disabled=busy on:click=confirm>
                        {move || if busy.get() { "Deleting…" } else { "Delete pipeline" }}
                    </button>
                </div>
            </div>
        </Dialog>
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
fn LabelledSelect(
    label: String,
    placeholder: String,
    value: ReadSignal<Option<Uuid>>,
    set_value: WriteSignal<Option<Uuid>>,
    options: Memo<Vec<(Uuid, String)>>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <select
                class="input"
                on:change=move |e| set_value.set(parse_uuid_or_none(&event_target_value(&e)))
            >
                <option value="">{placeholder.clone()}</option>
                {move || options.get().into_iter().map(|(id, lab)| {
                    let selected = value.get() == Some(id);
                    view! { <option value=id.to_string() selected=selected>{lab}</option> }
                }).collect_view()}
            </select>
        </label>
    }
}

fn short_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}
