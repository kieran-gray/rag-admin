use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::server_functions::source_document::requeue_embedding;
use crate::shared::contracts::{IndexingDto, PipelineConfigurationDto};
use crate::ui::components::primitives::{EmptyState, Help, Status, StatusPill, Surface};
use crate::ui::pages::document_detail::steps::{
    pipeline_meta_view, ConfigSelection, PipelineMetaDto,
};

#[component]
pub fn EmbedStep(
    selection: ConfigSelection,
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    indexings: Signal<Vec<IndexingDto>>,
    on_back: Callback<()>,
    on_advance: Callback<()>,
) -> impl IntoView {
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let active_indexing: Signal<Option<IndexingDto>> = Signal::derive(move || {
        let pid = selection.pipeline_id.get()?;
        indexings
            .get()
            .into_iter()
            .find(|ix| ix.pipeline_configuration_id == pid && !ix.removed)
    });

    let pipeline_meta: Signal<Option<PipelineMetaDto>> = Signal::derive(move || {
        let pid = selection.pipeline_id.get()?;
        pipelines.with_value(|ps| {
            ps.iter()
                .find(|p| p.pipeline_configuration_id == pid)
                .map(|p| PipelineMetaDto {
                    name: p.name.clone(),
                    embedding_model: p.embedding_model_name.clone(),
                    vector_index: p.vector_index_name.clone(),
                })
        })
    });

    let stage: Signal<EmbedStage> =
        Signal::derive(move || EmbedStage::from(active_indexing.get().as_ref()));

    let run = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(ix) = active_indexing.get() else {
            set_error.set(Some("Run chunking first.".into()));
            return;
        };
        if ix.chunk_set_id.is_none() {
            set_error.set(Some("Chunking has not completed yet.".into()));
            return;
        }
        let id = ix.indexing_id;
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            if let Err(e) = requeue_embedding(id).await {
                set_error.set(Some(format!("{e}")));
            }
            set_busy.set(false);
        });
    };

    let primary_label = Signal::derive(move || {
        if busy.get() {
            return "Submitting…".to_string();
        }
        match stage.get() {
            EmbedStage::AwaitingChunk => "Waiting on chunking…".to_string(),
            EmbedStage::Ready => "Run embedding".to_string(),
            EmbedStage::Running => "Embedding…".to_string(),
            EmbedStage::Done => "Re-run embedding".to_string(),
            EmbedStage::Failed => "Retry embedding".to_string(),
        }
    });

    let primary_disabled = move || {
        busy.get() || matches!(stage.get(), EmbedStage::AwaitingChunk | EmbedStage::Running)
    };

    let advance_disabled = move || !matches!(stage.get(), EmbedStage::Done);

    view! {
        <div class="space-y-6">
            <Surface
                title="Embed chunks".to_string()
                actions=Box::new(move || view! {
                    <Help title="What does embedding do?".to_string()>
                        <p>
                            "Each chunk is sent through the pipeline's embedding model, which returns a fixed-length vector that captures the chunk's meaning. Those vectors are what the search step compares against at query time."
                        </p>
                        <p class="mt-3">
                            "Embedding is purely mechanical: the chunks do not change. Re-running with the same chunks just regenerates the vectors using the same model."
                        </p>
                    </Help>
                }.into_any())
            >
                {move || match active_indexing.get() {
                    None => view! {
                        <EmptyState
                            title="Nothing to embed yet"
                            body="Chunk this document first.".to_string()
                            action=Box::new(move || view! {
                                <button
                                    type="button"
                                    class="btn"
                                    on:click=move |_| on_back.run(())
                                >
                                    "← Back to chunking"
                                </button>
                            }.into_any())
                        />
                    }.into_any(),
                    Some(ix) => {
                        let pm = pipeline_meta.get();
                        let document_version = ix.document_version;
                        let target = pm.as_ref().and_then(|m| m.embedding_model.clone());
                        view! {
                            <div class="space-y-4">
                                {pipeline_meta_view(pm, document_version, "Embedding model", target)}

                                <div class="flex items-center justify-between gap-3 pt-3 border-t border-[var(--color-border)]">
                                    <EmbedStatusLine status=stage />
                                    <button
                                        type="button"
                                        class="btn btn-primary"
                                        disabled=primary_disabled
                                        on:click=run
                                    >
                                        {move || primary_label.get()}
                                    </button>
                                </div>
                            </div>
                        }.into_any()
                    }
                }}

                {move || error.get().map(|e| view! {
                    <div class="log-line-error text-sm mt-3">{e}</div>
                })}
            </Surface>

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Next"</span>
                    <span class="step-advance-eyebrow-label">"Upsert vectors into the vector index"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=advance_disabled
                    on:click=move |_| on_advance.run(())
                >
                    "Continue to indexing →"
                </button>
            </div>
        </div>
    }
}

#[component]
fn EmbedStatusLine(status: Signal<EmbedStage>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 text-xs">
            {move || match status.get() {
                EmbedStage::AwaitingChunk => view! {
                    <StatusPill label="Waiting on chunking".to_string() kind=Status::Stale />
                }.into_any(),
                EmbedStage::Ready => view! {
                    <StatusPill label="Not started".to_string() kind=Status::Stale />
                }.into_any(),
                EmbedStage::Running => view! {
                    <StatusPill label="Embedding…".to_string() kind=Status::Pending />
                }.into_any(),
                EmbedStage::Done => view! {
                    <StatusPill label="Embedded".to_string() kind=Status::Ok />
                }.into_any(),
                EmbedStage::Failed => view! {
                    <StatusPill label="Failed".to_string() kind=Status::Fail />
                }.into_any(),
            }}
        </div>
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedStage {
    AwaitingChunk,
    Ready,
    Running,
    Done,
    Failed,
}

impl EmbedStage {
    pub fn from(ix: Option<&IndexingDto>) -> Self {
        let Some(ix) = ix else {
            return Self::AwaitingChunk;
        };
        if ix.status.contains("Failed") {
            return Self::Failed;
        }
        match (ix.chunk_set_id, ix.embedding_set_id) {
            (None, _) => Self::AwaitingChunk,
            (Some(_), None) => {
                if ix.status.contains("Embedding") {
                    Self::Running
                } else {
                    Self::Ready
                }
            }
            (Some(_), Some(_)) => Self::Done,
        }
    }
}
