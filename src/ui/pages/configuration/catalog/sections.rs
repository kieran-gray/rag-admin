use leptos::prelude::*;

use crate::shared::contracts::ConfigurationDto;
use crate::ui::components::primitives::{EmptyState, Surface};
use crate::ui::pages::configuration::commands::short_uuid;

use super::{DeleteTarget, RegistryForm};

#[component]
pub(super) fn EmbeddingModelsSection(
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
                            body="Register the embedding models you want index profiles to use.".to_string()
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
pub(super) fn GenerationModelsSection(
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
pub(super) fn VectorIndexesSection(
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
