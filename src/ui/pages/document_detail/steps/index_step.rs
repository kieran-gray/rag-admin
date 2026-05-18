use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use crate::server_functions::source_document::requeue_indexing;
use crate::shared::contracts::{IndexingDto, PipelineConfigurationDto};
use crate::ui::components::primitives::{EmptyState, Help, Status, StatusPill, Surface};
use crate::ui::pages::document_detail::steps::{
    pipeline_meta_view, ConfigSelection, PipelineMetaDto,
};

#[component]
pub fn IndexStep(
    selection: ConfigSelection,
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    indexings: Signal<Vec<IndexingDto>>,
    on_back: Callback<()>,
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

    let stage: Signal<IndexStage> =
        Signal::derive(move || IndexStage::from(active_indexing.get().as_ref()));

    let run = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(ix) = active_indexing.get() else {
            set_error.set(Some("Embed first.".into()));
            return;
        };
        if ix.embedding_set_id.is_none() {
            set_error.set(Some("Embedding has not completed yet.".into()));
            return;
        }
        let id = ix.indexing_id;
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            if let Err(e) = requeue_indexing(id).await {
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
            IndexStage::AwaitingEmbed => "Waiting on embedding…".to_string(),
            IndexStage::Ready => "Run indexing".to_string(),
            IndexStage::Running => "Indexing…".to_string(),
            IndexStage::Done => "Re-run indexing".to_string(),
            IndexStage::Failed => "Retry indexing".to_string(),
        }
    });

    let primary_disabled = move || {
        busy.get() || matches!(stage.get(), IndexStage::AwaitingEmbed | IndexStage::Running)
    };

    view! {
        <div class="space-y-6">
            <Surface
                title="Index vectors".to_string()
                actions=Box::new(move || view! {
                    <Help title="What does indexing do?".to_string()>
                        <p>
                            "Indexing upserts the embedded vectors into the pipeline's vector store. After this completes the document is searchable from the playground or directly via the API."
                        </p>
                        <p class="mt-3">
                            "Re-running indexing replaces the previously upserted vectors for this document. The vector store remains consistent: there will not be duplicates across runs."
                        </p>
                    </Help>
                }.into_any())
            >
                {move || match active_indexing.get() {
                    None => view! {
                        <EmptyState
                            title="Nothing to index yet"
                            body="Embed the chunks first.".to_string()
                            action=Box::new(move || view! {
                                <button
                                    type="button"
                                    class="btn"
                                    on:click=move |_| on_back.run(())
                                >
                                    "← Back to embedding"
                                </button>
                            }.into_any())
                        />
                    }.into_any(),
                    Some(ix) => {
                        let pm = pipeline_meta.get();
                        let document_version = ix.document_version;
                        let target = pm.as_ref().and_then(|m| m.vector_index.clone());
                        view! {
                            <div class="space-y-4">
                                {pipeline_meta_view(pm, document_version, "Vector index", target)}

                                <div class="flex items-center justify-between gap-3 pt-3 border-t border-[var(--color-border)]">
                                    <IndexStatusLine status=stage />
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

            {move || matches!(stage.get(), IndexStage::Done).then(|| view! {
                <div class="done-banner">
                    <div class="flex items-center gap-3">
                        <StatusPill label="Indexed".to_string() kind=Status::Ok />
                        <span class="text-sm">"This document is searchable."</span>
                    </div>
                    <div class="done-banner-actions">
                        <A href="/playground/retrieve" attr:class="btn btn-sm btn-ghost">
                            "Open in playground →"
                        </A>
                    </div>
                </div>
            })}
        </div>
    }
}

#[component]
fn IndexStatusLine(status: Signal<IndexStage>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 text-xs">
            {move || match status.get() {
                IndexStage::AwaitingEmbed => view! {
                    <StatusPill label="Waiting on embedding".to_string() kind=Status::Stale />
                }.into_any(),
                IndexStage::Ready => view! {
                    <StatusPill label="Not started".to_string() kind=Status::Stale />
                }.into_any(),
                IndexStage::Running => view! {
                    <StatusPill label="Indexing…".to_string() kind=Status::Pending />
                }.into_any(),
                IndexStage::Done => view! {
                    <StatusPill label="Indexed".to_string() kind=Status::Ok />
                }.into_any(),
                IndexStage::Failed => view! {
                    <StatusPill label="Failed".to_string() kind=Status::Fail />
                }.into_any(),
            }}
        </div>
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStage {
    AwaitingEmbed,
    Ready,
    Running,
    Done,
    Failed,
}

impl IndexStage {
    pub fn from(ix: Option<&IndexingDto>) -> Self {
        let Some(ix) = ix else {
            return Self::AwaitingEmbed;
        };
        if ix.status.contains("Failed") {
            return Self::Failed;
        }
        if ix.status.contains("Indexed") {
            return Self::Done;
        }
        match ix.embedding_set_id {
            None => Self::AwaitingEmbed,
            Some(_) => {
                if ix.status.contains("Indexing") {
                    Self::Running
                } else {
                    Self::Ready
                }
            }
        }
    }
}
