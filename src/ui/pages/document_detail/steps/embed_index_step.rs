use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::contracts::{IndexingDto, PipelineConfigurationDto};
use crate::server_functions::source_document::{requeue_embedding, requeue_indexing};
use crate::ui::components::activity::toggle_drawer;
use crate::ui::components::primitives::{EmptyState, Status, StatusPill, Surface};
use crate::ui::pages::document_detail::steps::ConfigSelection;

#[component]
pub fn EmbedIndexStep(
    selection: ConfigSelection,
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    indexings: Signal<Vec<IndexingDto>>,
    on_back: Callback<()>,
) -> impl IntoView {
    let (busy_stage, set_busy_stage) = signal::<Option<&'static str>>(None);
    let (error, set_error) = signal::<Option<String>>(None);
    let (auto_finish, set_auto_finish) = signal(false);
    let auto_followed_up = StoredValue::new(false);

    let active_indexing: Signal<Option<IndexingDto>> = Signal::derive(move || {
        let pid = selection.pipeline_id.get()?;
        indexings
            .get()
            .into_iter()
            .find(|ix| ix.pipeline_configuration_id == pid && !ix.removed)
    });

    let pipeline_info: Signal<Option<PipelineSummary>> = Signal::derive(move || {
        let pid = selection.pipeline_id.get()?;
        pipelines.with_value(|ps| {
            ps.iter()
                .find(|p| p.pipeline_configuration_id == pid)
                .map(|p| PipelineSummary {
                    name: p.name.clone(),
                    embedding_model: p.embedding_model_name.clone(),
                    vector_index: p.vector_index_name.clone(),
                })
        })
    });

    let stage: Signal<Stage> = Signal::derive(move || Stage::from(active_indexing.get().as_ref()));

    Effect::new(move |_| {
        let current = stage.get();
        let id_opt = active_indexing.get().map(|ix| ix.indexing_id);

        if matches!(current, Stage::Embedded) && auto_finish.get() && !auto_followed_up.get_value()
        {
            if let Some(id) = id_opt {
                auto_followed_up.set_value(true);
                set_busy_stage.set(Some("index"));
                set_error.set(None);
                spawn_local(async move {
                    match requeue_indexing(id).await {
                        Ok(_) => {
                            set_busy_stage.set(None);
                            set_auto_finish.set(false);
                        }
                        Err(e) => {
                            set_busy_stage.set(None);
                            set_auto_finish.set(false);
                            set_error.set(Some(format!("{e}")));
                        }
                    }
                });
            }
        }

        if matches!(current, Stage::Indexed | Stage::Failed) {
            auto_followed_up.set_value(false);
        }
    });

    let run_embed = move |_| {
        if busy_stage.get_untracked().is_some() {
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
        auto_followed_up.set_value(false);
        set_busy_stage.set(Some("embed"));
        set_error.set(None);
        set_auto_finish.set(true);
        spawn_local(async move {
            match requeue_embedding(id).await {
                Ok(_) => {
                    toggle_drawer(true);
                    set_busy_stage.set(None);
                }
                Err(e) => {
                    set_busy_stage.set(None);
                    set_auto_finish.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    let run_index = move |_| {
        if busy_stage.get_untracked().is_some() {
            return;
        }
        let Some(ix) = active_indexing.get() else {
            return;
        };
        let id = ix.indexing_id;
        set_busy_stage.set(Some("index"));
        set_error.set(None);
        spawn_local(async move {
            match requeue_indexing(id).await {
                Ok(_) => {
                    toggle_drawer(true);
                    set_busy_stage.set(None);
                }
                Err(e) => {
                    set_busy_stage.set(None);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    let primary_label = Signal::derive(move || {
        if busy_stage.get() == Some("embed") {
            "Embedding…".to_string()
        } else if busy_stage.get() == Some("index") {
            "Indexing…".to_string()
        } else {
            match stage.get() {
                Stage::AwaitingChunk => "Waiting on chunking…".to_string(),
                Stage::ChunkReady => "Start embed & index".to_string(),
                Stage::Embedding => "Embedding in progress…".to_string(),
                Stage::Embedded => "Resume — index now".to_string(),
                Stage::Indexing => "Upserting vectors…".to_string(),
                Stage::Indexed => "Re-run embed & index".to_string(),
                Stage::Failed => "Retry from embed".to_string(),
            }
        }
    });

    let on_primary = move |ev: leptos::ev::MouseEvent| {
        if matches!(stage.get_untracked(), Stage::Embedded) {
            run_index(ev);
        } else {
            run_embed(ev);
        }
    };

    let primary_disabled = move || {
        busy_stage.get().is_some()
            || matches!(
                stage.get(),
                Stage::AwaitingChunk | Stage::Embedding | Stage::Indexing
            )
    };

    view! {
        <div class="space-y-6">
            <Surface title="Embed & index".to_string()>
                <p class="text-sm muted mb-4">
                    "Two things still need to happen for this document to be searchable. They run back-to-back — kick off "
                    <span class="text-text font-medium">"Embed"</span>
                    " and "
                    <span class="text-text font-medium">"Index"</span>
                    " continues automatically when it completes."
                </p>

                {move || match active_indexing.get() {
                    None => view! {
                        <EmptyState
                            title="Nothing to embed yet"
                            body="Run chunking in step 2 first.".to_string()
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
                        let pipeline = pipeline_info.get();
                        let document_version = ix.document_version;
                        view! {
                            <div class="space-y-4">
                                <PipelineMeta pipeline=pipeline document_version=document_version />

                                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                                    <SubStepCard
                                        ordinal="A"
                                        title="Embed"
                                        description="Run each chunk through the pipeline's embedding model to produce a vector.".to_string()
                                        target_label=pipeline_info.get().and_then(|p| p.embedding_model)
                                        state=Signal::derive(move || embed_state(stage.get(), busy_stage.get()))
                                    />
                                    <SubStepCard
                                        ordinal="B"
                                        title="Index"
                                        description="Upsert the resulting vectors into the pipeline's vector store. Runs automatically after embed completes.".to_string()
                                        target_label=pipeline_info.get().and_then(|p| p.vector_index)
                                        state=Signal::derive(move || index_state(stage.get(), busy_stage.get(), auto_finish.get()))
                                    />
                                </div>

                                <div class="flex items-center justify-between gap-3 pt-3 border-t border-[var(--color-border)]">
                                    <span class="text-xs muted">
                                        {move || stage_hint(stage.get(), busy_stage.get(), auto_finish.get())}
                                    </span>
                                    <button
                                        type="button"
                                        class="btn btn-primary"
                                        disabled=primary_disabled
                                        on:click=on_primary
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
        </div>
    }
}

#[derive(Clone)]
struct PipelineSummary {
    name: String,
    embedding_model: Option<String>,
    vector_index: Option<String>,
}

#[component]
fn PipelineMeta(pipeline: Option<PipelineSummary>, document_version: u32) -> impl IntoView {
    view! {
        <div class="flex items-center gap-3 flex-wrap text-sm">
            <span class="eyebrow">"Pipeline"</span>
            <span class="font-medium">
                {pipeline.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "—".to_string())}
            </span>
            <span class="muted">{format!("· doc v{document_version}")}</span>
        </div>
    }
}

#[component]
fn SubStepCard(
    ordinal: &'static str,
    title: &'static str,
    description: String,
    target_label: Option<String>,
    state: Signal<SubStepState>,
) -> impl IntoView {
    view! {
        <div class="surface-raised rounded p-4 flex flex-col gap-2">
            <div class="flex items-center justify-between gap-3">
                <div class="flex items-center gap-2">
                    <span class=move || format!(
                        "inline-flex items-center justify-center w-6 h-6 rounded-full text-[11px] font-mono {}",
                        match state.get() {
                            SubStepState::Done => "bg-[var(--color-accent-soft)] text-[var(--color-accent)] border border-[var(--color-accent)]",
                            SubStepState::Active => "bg-[var(--status-pending)] text-[var(--color-page-bg)]",
                            SubStepState::Failed => "bg-transparent text-[var(--status-fail)] border border-[var(--status-fail)]",
                            SubStepState::Pending => "bg-transparent text-[var(--color-text-muted)] border border-[var(--color-border)]",
                        }
                    )>
                        {ordinal}
                    </span>
                    <span class="text-sm font-semibold">{title}</span>
                </div>
                {move || sub_step_pill(state.get())}
            </div>
            <p class="text-xs muted">{description}</p>
            {target_label.map(|name| view! {
                <div class="text-[11px] muted">
                    <span class="eyebrow">"Target"</span>
                    <span class="ml-2 font-mono text-text">{name}</span>
                </div>
            })}
        </div>
    }
}

fn sub_step_pill(state: SubStepState) -> AnyView {
    match state {
        SubStepState::Done => view! {
            <StatusPill label="Done".to_string() kind=Status::Ok />
        }
        .into_any(),
        SubStepState::Active => view! {
            <StatusPill label="Running".to_string() kind=Status::Pending />
        }
        .into_any(),
        SubStepState::Pending => view! {
            <StatusPill label="Not started".to_string() kind=Status::Stale />
        }
        .into_any(),
        SubStepState::Failed => view! {
            <StatusPill label="Failed".to_string() kind=Status::Fail />
        }
        .into_any(),
    }
}

fn embed_state(stage: Stage, busy: Option<&'static str>) -> SubStepState {
    match stage {
        Stage::AwaitingChunk => SubStepState::Pending,
        Stage::ChunkReady => {
            if busy == Some("embed") {
                SubStepState::Active
            } else {
                SubStepState::Pending
            }
        }
        Stage::Embedding => SubStepState::Active,
        Stage::Embedded | Stage::Indexing | Stage::Indexed => SubStepState::Done,
        Stage::Failed => SubStepState::Failed,
    }
}

fn index_state(stage: Stage, busy: Option<&'static str>, auto_finish: bool) -> SubStepState {
    match stage {
        Stage::AwaitingChunk | Stage::ChunkReady | Stage::Embedding => SubStepState::Pending,
        Stage::Embedded => {
            if busy == Some("index") || auto_finish {
                SubStepState::Active
            } else {
                SubStepState::Pending
            }
        }
        Stage::Indexing => SubStepState::Active,
        Stage::Indexed => SubStepState::Done,
        Stage::Failed => SubStepState::Pending,
    }
}

fn stage_hint(stage: Stage, busy: Option<&'static str>, auto_finish: bool) -> String {
    match stage {
        Stage::AwaitingChunk => "Chunking hasn't finished yet.".to_string(),
        Stage::ChunkReady if busy == Some("embed") => "Submitting embed request…".to_string(),
        Stage::ChunkReady => "Click below — both Embed and Index will run.".to_string(),
        Stage::Embedding => "Embedding in flight. Index follows automatically.".to_string(),
        Stage::Embedded if busy == Some("index") => "Indexing started.".to_string(),
        Stage::Embedded if auto_finish => {
            "Embed done — kicking off index automatically.".to_string()
        }
        Stage::Embedded => {
            "Embed finished. Click below to run the index step manually.".to_string()
        }
        Stage::Indexing => "Upserting vectors into the vector store…".to_string(),
        Stage::Indexed => {
            "Document is searchable. Re-run if the chunks or pipeline change.".to_string()
        }
        Stage::Failed => "Something failed. Retry to start again from embed.".to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubStepState {
    Pending,
    Active,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    AwaitingChunk,
    ChunkReady,
    Embedding,
    Embedded,
    Indexing,
    Indexed,
    Failed,
}

impl Stage {
    fn from(ix: Option<&IndexingDto>) -> Self {
        let Some(ix) = ix else {
            return Self::AwaitingChunk;
        };
        if ix.status.contains("Failed") {
            return Self::Failed;
        }
        match (ix.chunk_set_id, ix.embedding_set_id, ix.status.as_str()) {
            (None, _, _) => Self::AwaitingChunk,
            (Some(_), None, s) if s.contains("Pending") || s.contains("Chunking") => {
                if s.contains("Embedding") {
                    Self::Embedding
                } else {
                    Self::ChunkReady
                }
            }
            (Some(_), None, _) => Self::Embedding,
            (Some(_), Some(_), s) if s.contains("Indexed") => Self::Indexed,
            (Some(_), Some(_), s) if s.contains("Embedding") => Self::Embedded,
            (Some(_), Some(_), _) => Self::Indexing,
        }
    }
}
