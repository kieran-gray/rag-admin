use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::components::activity::toggle_drawer;
use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{EmptyState, Status, StatusPill, Surface};
use crate::server_functions::source_document::{
    request_indexing, requeue_chunking, requeue_embedding, requeue_indexing,
    start_source_document_ingest,
};
use crate::shared::{
    aggregate_type, ChunkingConfig, ChunkingConfigurationDto, IndexingDto,
    PipelineConfigurationDto, SourceDocumentDetailDto,
};

#[component]
pub fn IndexingsTab(
    detail: Option<SourceDocumentDetailDto>,
    pipelines: Vec<PipelineConfigurationDto>,
    source_ref: String,
) -> impl IntoView {
    let pipelines_stored = StoredValue::new(pipelines);
    let source_ref_stored = StoredValue::new(source_ref);

    let invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::INDEXING,
            aggregate_type::EMBEDDING_MODEL_CATALOG,
            aggregate_type::GENERATION_MODEL_CATALOG,
            aggregate_type::VECTOR_INDEX_CATALOG,
            aggregate_type::SWEEP_TEMPLATE,
        ])
    });
    let chunking_configurations = Resource::new(
        move || invalidator.get(),
        |_| async move {
            crate::server_functions::configuration::get_chunking_configurations()
                .await
                .unwrap_or_default()
        },
    );

    let indexings = detail.map(|d| d.indexings).unwrap_or_default();

    view! {
        <div class="grid grid-cols-1 lg:grid-cols-[1fr_360px] gap-6">
            <div>
                <Surface flush=true>
                    {if indexings.is_empty() {
                        view! {
                            <div class="p-6">
                                <EmptyState
                                    title="No indexings yet"
                                    body="Pick a pipeline and chunking configuration on the right to set up the first indexing.".to_string()
                                />
                            </div>
                        }.into_any()
                    } else {
                        view! { <IndexingsTable indexings=indexings /> }.into_any()
                    }}
                </Surface>
            </div>
            <div>
                <Transition fallback=|| view! { <p class="muted">"Loading chunking library…"</p> }>
                    {move || chunking_configurations.get().map(|chunking_list| view! {
                        <NewIndexingPanel
                            pipelines=pipelines_stored
                            source_ref=source_ref_stored
                            chunking_configurations=StoredValue::new(chunking_list)
                        />
                    })}
                </Transition>
            </div>
        </div>
    }
}

#[component]
fn IndexingsTable(indexings: Vec<IndexingDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <thead>
                <tr>
                    <th>"Pipeline"</th>
                    <th>"Status"</th>
                    <th>"Stage controls"</th>
                    <th class="text-right">"Doc version"</th>
                </tr>
            </thead>
            <tbody>
                {indexings.into_iter().map(|ix| {
                    let (kind, label) = status_display(&ix.status);
                    let pipeline_short = ix.pipeline_configuration_id.to_string()[..8].to_string();
                    view! {
                        <tr>
                            <td>
                                <span class="font-mono text-xs muted">
                                    {format!("{pipeline_short}…")}
                                </span>
                            </td>
                            <td><StatusPill label=label.to_string() kind=kind /></td>
                            <td>
                                <StageControls ix=ix.clone() />
                            </td>
                            <td class="text-right font-mono text-xs muted">
                                {format!("v{}", ix.document_version)}
                            </td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn StageControls(ix: IndexingDto) -> impl IntoView {
    let indexing_id = ix.indexing_id;
    let status = ix.status.clone();
    let (running_stage, set_running_stage) = signal::<Option<&'static str>>(None);
    let (error, set_error) = signal::<Option<String>>(None);

    let make_runner = move |stage: &'static str, runner: fn(Uuid) -> _| {
        move |_| {
            if running_stage.get_untracked().is_some() {
                return;
            }
            set_running_stage.set(Some(stage));
            set_error.set(None);
            let fut = runner(indexing_id);
            spawn_local(async move {
                match fut.await {
                    Ok(()) => {
                        toggle_drawer(true);
                        set_running_stage.set(None);
                    }
                    Err(e) => {
                        set_running_stage.set(None);
                        set_error.set(Some(format!("{e}")));
                    }
                }
            });
        }
    };

    let chunk_done =
        status.contains("Chunking") || status.contains("Embedding") || status.contains("Indexed");
    let embed_done = status.contains("Embedding") || status.contains("Indexed");
    let indexed_done = status.contains("Indexed");

    let chunk_busy = move || running_stage.get() == Some("chunk");
    let embed_busy = move || running_stage.get() == Some("embed");
    let upsert_busy = move || running_stage.get() == Some("upsert");
    let any_busy = move || running_stage.get().is_some();

    let on_chunk = make_runner("chunk", |id| {
        Box::pin(requeue_chunking(id))
            as std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<(), leptos::prelude::ServerFnError>>
                        + Send,
                >,
            >
    });
    let on_embed = make_runner("embed", |id| {
        Box::pin(requeue_embedding(id))
            as std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<(), leptos::prelude::ServerFnError>>
                        + Send,
                >,
            >
    });
    let on_upsert = make_runner("upsert", |id| {
        Box::pin(requeue_indexing(id))
            as std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<(), leptos::prelude::ServerFnError>>
                        + Send,
                >,
            >
    });

    view! {
        <div class="flex flex-col gap-1.5">
            <div class="flex gap-1.5">
                <StageButton
                    label="Chunk"
                    done=chunk_done
                    busy=Signal::derive(chunk_busy)
                    any_busy=Signal::derive(any_busy)
                    on_click=Box::new(on_chunk)
                />
                <StageButton
                    label="Embed"
                    done=embed_done
                    busy=Signal::derive(embed_busy)
                    any_busy=Signal::derive(any_busy)
                    disabled_reason=Box::new(move || (!chunk_done).then(|| "Chunk first".to_string()))
                    on_click=Box::new(on_embed)
                />
                <StageButton
                    label="Index"
                    done=indexed_done
                    busy=Signal::derive(upsert_busy)
                    any_busy=Signal::derive(any_busy)
                    disabled_reason=Box::new(move || (!embed_done).then(|| "Embed first".to_string()))
                    on_click=Box::new(on_upsert)
                />
            </div>
            {move || error.get().map(|e| view! {
                <div class="log-line-error text-xs px-1">{e}</div>
            })}
        </div>
    }
}

#[component]
fn StageButton(
    label: &'static str,
    done: bool,
    #[prop(into)] busy: Signal<bool>,
    #[prop(into)] any_busy: Signal<bool>,
    #[prop(optional)] disabled_reason: Option<Box<dyn Fn() -> Option<String> + Send + Sync>>,
    on_click: Box<dyn Fn(leptos::ev::MouseEvent) + Send + Sync>,
) -> impl IntoView {
    let disabled_reason_stored = StoredValue::new(disabled_reason);
    let blocked = move || disabled_reason_stored.with_value(|f| f.as_ref().and_then(|f| f()));
    let on_click_stored = StoredValue::new(on_click);
    view! {
        <button
            type="button"
            class=move || {
                let base = "btn text-xs px-2 py-1";
                if done {
                    format!("{base} btn-ghost")
                } else if busy.get() {
                    format!("{base} btn-primary")
                } else if blocked().is_some() || any_busy.get() {
                    format!("{base} opacity-50")
                } else {
                    format!("{base} btn-primary")
                }
            }
            disabled=move || any_busy.get() || blocked().is_some()
            title=move || blocked().unwrap_or_default()
            on:click=move |ev| on_click_stored.with_value(|f| f(ev))
        >
            {move || if busy.get() {
                format!("{label}…")
            } else if done {
                format!("{label} ✓")
            } else {
                label.to_string()
            }}
        </button>
    }
}

#[component]
fn NewIndexingPanel(
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    source_ref: StoredValue<String>,
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
) -> impl IntoView {
    let (active_pipeline, set_active_pipeline) = signal::<Option<Uuid>>(None);
    let (active_chunking, set_active_chunking) = signal::<Option<Uuid>>(None);
    let (running, set_running) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        if active_chunking.get_untracked().is_none() {
            if let Some(first) = chunking_configurations
                .with_value(|c| c.first().map(|cc| cc.chunking_configuration_id))
            {
                set_active_chunking.set(Some(first));
            }
        }
    });

    let resolve_chunking_config = move || -> Option<ChunkingConfig> {
        let id = active_chunking.get()?;
        chunking_configurations.with_value(|c| {
            c.iter()
                .find(|cc| cc.chunking_configuration_id == id)
                .map(|cc| cc.config)
        })
    };

    let can_start = move || {
        active_pipeline.get().is_some() && active_chunking.get().is_some() && !running.get()
    };

    let on_create_stepwise = move |_| {
        let (Some(pipeline_id), Some(config)) = (active_pipeline.get(), resolve_chunking_config())
        else {
            return;
        };
        let slug = source_ref.get_value();
        set_running.set(true);
        set_error.set(None);
        spawn_local(async move {
            match request_indexing(slug, pipeline_id, config, false).await {
                Ok(_indexing_id) => {
                    toggle_drawer(true);
                    set_running.set(false);
                }
                Err(e) => {
                    set_running.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    let on_run_full = move |_| {
        let (Some(pipeline_id), Some(config)) = (active_pipeline.get(), resolve_chunking_config())
        else {
            return;
        };
        let slug = source_ref.get_value();
        set_running.set(true);
        set_error.set(None);
        spawn_local(async move {
            match start_source_document_ingest(slug, pipeline_id, config).await {
                Ok(_indexing_id) => {
                    toggle_drawer(true);
                    set_running.set(false);
                }
                Err(e) => {
                    set_running.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    view! {
        <Surface title="New indexing".to_string()>
            <div class="space-y-4">
                <div>
                    <div class="eyebrow mb-2">"Pipeline"</div>
                    <div class="space-y-1.5">
                        {move || {
                            let ps = pipelines.get_value();
                            if ps.is_empty() {
                                return view! {
                                    <p class="muted text-sm">
                                        "No pipelines configured — add one in Pipelines."
                                    </p>
                                }.into_any();
                            }
                            ps.into_iter().map(|pc| {
                                let pc_id = pc.pipeline_configuration_id;
                                let active = move || active_pipeline.get() == Some(pc_id);
                                view! {
                                    <button
                                        type="button"
                                        class=move || format!(
                                            "w-full text-left px-3 py-2 rounded border text-sm transition-colors {}",
                                            if active() {
                                                "border-[var(--color-accent)] text-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                                            } else {
                                                "border-[var(--color-border)] hover:border-[var(--color-border-strong)]"
                                            }
                                        )
                                        on:click=move |_| set_active_pipeline.set(Some(pc_id))
                                    >
                                        {pc.name}
                                    </button>
                                }
                            }).collect_view().into_any()
                        }}
                    </div>
                </div>

                <div>
                    <div class="eyebrow mb-2">"Chunking configuration"</div>
                    {move || {
                        let cs = chunking_configurations.get_value();
                        if cs.is_empty() {
                            return view! {
                                <p class="muted text-sm">
                                    "No chunking configurations — add some in Chunking."
                                </p>
                            }.into_any();
                        }
                        view! {
                            <select
                                class="input"
                                on:change=move |e| {
                                    let v = event_target_value(&e);
                                    set_active_chunking.set(Uuid::parse_str(&v).ok());
                                }
                            >
                                {cs.into_iter().map(|cc| {
                                    let id = cc.chunking_configuration_id;
                                    let label = format!("{} · {}", cc.name, cc.config.describe());
                                    let selected = active_chunking.get() == Some(id);
                                    view! {
                                        <option value=id.to_string() selected=selected>{label}</option>
                                    }
                                }).collect_view()}
                            </select>
                        }.into_any()
                    }}
                </div>

                <div class="flex flex-col gap-2 pt-2">
                    <button
                        type="button"
                        class="btn btn-primary w-full justify-center"
                        disabled=move || !can_start()
                        on:click=on_run_full
                    >
                        {move || if running.get() {
                            "Working…"
                        } else if active_pipeline.get().is_none() {
                            "Select a pipeline"
                        } else if active_chunking.get().is_none() {
                            "Select a chunking config"
                        } else {
                            "Run full ingest"
                        }}
                    </button>
                    <button
                        type="button"
                        class="btn w-full justify-center"
                        disabled=move || !can_start()
                        on:click=on_create_stepwise
                    >
                        "Chunk only (stepwise)"
                    </button>
                    <p class="text-xs faint">
                        "Run full does chunk → embed → index automatically.
                         Stepwise chunks now and stops; advance with the per-row buttons when you're happy with the chunks."
                    </p>
                </div>

                {move || error.get().map(|e| view! {
                    <div class="log-line-error text-sm">{e}</div>
                })}
            </div>
        </Surface>
    }
}

fn status_display(status: &str) -> (Status, &'static str) {
    if status.contains("Indexed") {
        (Status::Ok, "Indexed")
    } else if status.contains("Failed") {
        (Status::Fail, "Failed")
    } else if status.contains("Embedding") {
        (Status::Info, "Embedded")
    } else if status.contains("Chunking") {
        (Status::Info, "Chunked")
    } else {
        (Status::Pending, "Pending")
    }
}
