mod form_dialog;
mod sections;
mod widgets;

use leptos::prelude::*;

use crate::server_functions::configuration::get_configuration;
use crate::shared::contracts::{
    aggregate_type, EmbeddingModelCommandDto, EmbeddingModelDto, GenerationModelCommandDto,
    GenerationModelDto, VectorIndexCommandDto, VectorIndexDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{InlineStatusMessage, PageHeader, Surface};
use crate::ui::pages::configuration::commands::{
    run_embedding_model_command, run_generation_model_command, run_vector_index_command,
};

use self::form_dialog::{RegistryDeleteDialog, RegistryFormDialog};
use self::sections::{EmbeddingModelsSection, GenerationModelsSection, VectorIndexesSection};
use self::widgets::StatusBanner;

#[derive(Clone)]
pub(super) enum CatalogCommand {
    Embedding(EmbeddingModelCommandDto),
    Generation(GenerationModelCommandDto),
    VectorIndex(VectorIndexCommandDto),
}

#[derive(Clone)]
pub(super) enum RegistryForm {
    AddEmbeddingModel,
    EditEmbeddingModel(EmbeddingModelDto),
    AddGenerationModel,
    EditGenerationModel(GenerationModelDto),
    AddVectorIndex,
    EditVectorIndex(VectorIndexDto),
}

#[derive(Clone)]
pub(super) enum DeleteTarget {
    EmbeddingModel(EmbeddingModelDto),
    GenerationModel(GenerationModelDto),
    VectorIndex(VectorIndexDto),
}

impl DeleteTarget {
    pub(super) fn label(&self) -> String {
        match self {
            Self::EmbeddingModel(m) => format!("Embedding model · {}", m.model),
            Self::GenerationModel(m) => format!("Generation model · {}", m.model),
            Self::VectorIndex(i) => format!("Vector index · {}", i.name),
        }
    }
}

pub(super) fn dispatch_catalog_command<F: FnOnce() + 'static>(
    command: CatalogCommand,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
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
pub fn CatalogPage() -> impl IntoView {
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

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);

    view! {
        <div>
            <PageHeader
                title="Catalog"
                subtitle="Models and vector indexes that pipelines and chunking configurations compose.".to_string()
            />

            <StatusBanner status=status />

            <Transition fallback=|| view! { <p class="muted">"Loading settings…"</p> }>
                {move || configuration.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error">{format!("Failed to load registry: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(cfg) => {
                        let config = StoredValue::new(cfg);
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
                        }.into_any()
                    }
                })}
            </Transition>
        </div>
    }
}
