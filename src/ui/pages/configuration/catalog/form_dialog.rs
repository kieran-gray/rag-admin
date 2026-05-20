use leptos::prelude::*;

use crate::shared::contracts::{
    AddEmbeddingModelDto, AddGenerationModelDto, AddVectorIndexDto, EmbeddingModelCommandDto,
    GenerationModelCommandDto, RemoveEmbeddingModelDto, RemoveGenerationModelDto,
    RemoveVectorIndexDto, UpdateEmbeddingModelDto, UpdateGenerationModelDto, UpdateVectorIndexDto,
    VectorIndexCommandDto,
};
use crate::shared::reference_data::{AiProviderKind, VectorStoreKind};
use crate::ui::components::primitives::{Dialog, InlineStatusMessage};

use super::widgets::{AiKindSelect, LabelledInput, LabelledNum, VectorKindSelect};
use super::{dispatch_catalog_command, CatalogCommand, DeleteTarget, RegistryForm};

#[component]
pub(super) fn RegistryFormDialog(
    form: ReadSignal<Option<RegistryForm>>,
    set_form: WriteSignal<Option<RegistryForm>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    let (name, set_name) = signal(String::new());
    let (ai_kind, set_ai_kind) = signal(AiProviderKind::Cloudflare);
    let (vector_kind, set_vector_kind) = signal(VectorStoreKind::CloudflareVectorize);
    let (model_id, set_model_id) = signal(String::new());
    let (dims, set_dims) = signal(1024u32);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form.get() {
            None => {}
            Some(RegistryForm::AddEmbeddingModel) => {
                set_ai_kind.set(AiProviderKind::Cloudflare);
                set_model_id.set(String::new());
                set_dims.set(1024);
            }
            Some(RegistryForm::EditEmbeddingModel(m)) => {
                set_ai_kind.set(m.kind);
                set_model_id.set(m.model);
                set_dims.set(m.dimensions);
            }
            Some(RegistryForm::AddGenerationModel) => {
                set_ai_kind.set(AiProviderKind::Cloudflare);
                set_model_id.set(String::new());
            }
            Some(RegistryForm::EditGenerationModel(m)) => {
                set_ai_kind.set(m.kind);
                set_model_id.set(m.model);
            }
            Some(RegistryForm::AddVectorIndex) => {
                set_vector_kind.set(VectorStoreKind::CloudflareVectorize);
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
            &name.get_untracked(),
            ai_kind.get_untracked(),
            vector_kind.get_untracked(),
            &model_id.get_untracked(),
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
            title=Signal::derive(move || form_title(form.get().as_ref())).get()
            subtitle=Signal::derive(move || form_subtitle(form.get().as_ref())).get()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|m| view! {
                    <div class="log-line-error text-sm">{m}</div>
                })}

                {move || match form.get() {
                    None => ().into_any(),
                    Some(RegistryForm::AddEmbeddingModel | RegistryForm::EditEmbeddingModel(_)) => view! {
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
                    Some(RegistryForm::AddGenerationModel | RegistryForm::EditGenerationModel(_)) => view! {
                        <AiKindSelect value=ai_kind set_value=set_ai_kind />
                        <LabelledInput
                            label="Model ID".to_string()
                            hint="Chat/completion model identifier".to_string()
                            value=model_id
                            set_value=set_model_id
                        />
                    }.into_any(),
                    Some(RegistryForm::AddVectorIndex | RegistryForm::EditVectorIndex(_)) => view! {
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
pub(super) fn RegistryDeleteDialog(
    target: ReadSignal<Option<DeleteTarget>>,
    set_target: WriteSignal<Option<DeleteTarget>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
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

fn build_command(
    form: RegistryForm,
    name: &str,
    ai_kind: AiProviderKind,
    vector_kind: VectorStoreKind,
    model_id: &str,
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

fn form_title(form: Option<&RegistryForm>) -> String {
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

fn form_subtitle(form: Option<&RegistryForm>) -> String {
    match form {
        None => String::new(),
        Some(RegistryForm::AddEmbeddingModel | RegistryForm::EditEmbeddingModel(_)) => {
            "Dimensions must match the target vector index.".into()
        }
        Some(RegistryForm::AddGenerationModel | RegistryForm::EditGenerationModel(_)) => {
            "Used by LLM-driven chunking and synthetic dataset generation.".into()
        }
        Some(RegistryForm::AddVectorIndex | RegistryForm::EditVectorIndex(_)) => {
            "Dimensions must match the embedding model that writes into it.".into()
        }
    }
}
