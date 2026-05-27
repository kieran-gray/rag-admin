use std::collections::BTreeMap;

use leptos::either::Either;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use super::map_phase::{section_count, MapPhase, MapProgress};
use crate::server_functions::comprehension::{
    get_document_map, rebuild_document_map, request_document_map_build,
};
use crate::server_functions::configuration::get_configuration;
use crate::server_functions::source_document::get_document_detail_by_source_ref;
use crate::shared::contracts::{
    aggregate_type, ConfigurationDto, DocumentMapDetailDto, InsightDto, MapItemRefDto,
    ObservationDto, RequestMapBuildRequestDto, SourceDocumentDetailDto, SpanDto, SuggestedRoleDto,
    ThreadDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    ActionItem, ActionsMenu, Dialog, EmptyState, PageHeader, StatusPill, Surface,
};

#[component]
pub fn DocumentMapPage() -> impl IntoView {
    let params = use_params_map();
    let source_ref =
        Memo::new(move |_| params.with(|p| p.get("source_ref").unwrap_or_default().to_string()));

    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::SOURCE_DOCUMENT]));
    let document_resource = Resource::new(
        move || (source_ref.get(), invalidator.get()),
        move |(slug, _)| async move {
            if slug.is_empty() {
                return Err("missing source_ref".to_string());
            }
            get_document_detail_by_source_ref(slug)
                .await
                .map_err(|e| e.to_string())
        },
    );

    view! {
        <div>
            <Transition fallback=|| view! {
                <p class="muted">"Loading document…"</p>
            }>
                {move || document_resource.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface><div class="log-line-error">{format!("Failed to load: {e}")}</div></Surface>
                    }.into_any(),
                    Ok(None) => view! {
                        <Surface>
                            <EmptyState
                                title="Document not found"
                                body="No document is registered at this source ref.".to_string()
                            />
                        </Surface>
                    }.into_any(),
                    Ok(Some(detail)) => view! {
                        <MapWorkspace detail=detail />
                    }.into_any(),
                })}
            </Transition>
        </div>
    }
}

#[component]
fn MapWorkspace(detail: SourceDocumentDetailDto) -> impl IntoView {
    let document = detail.document.clone();
    let document_id = document.document_id;
    let document_version = document.latest_version;
    let source_ref_key = document.source_ref_key.clone();
    let document_title = document.title.clone();

    let map_invalidator = use_invalidator(|e| e.from_any(&["document_map"]));
    let map_resource = Resource::new(
        move || (document_id, document_version, map_invalidator.get()),
        move |(id, ver, _)| async move { get_document_map(id, ver).await.map_err(|e| e.to_string()) },
    );

    let configuration = Resource::new(
        || (),
        |_| async move { get_configuration().await.map_err(|e| e.to_string()) },
    );

    let (busy, set_busy) = signal(false);
    let (build_error, set_build_error) = signal::<Option<String>>(None);
    let (selected_model, set_selected_model) = signal::<Option<Uuid>>(None);

    Effect::new(move |_| {
        if selected_model.get().is_none() {
            if let Some(Ok(config)) = configuration.get() {
                if let Some(first) = config.generation_models.first() {
                    set_selected_model.set(Some(first.generation_model_id));
                }
            }
        }
    });

    let on_build = move |_| {
        let Some(model_id) = selected_model.get() else {
            set_build_error.set(Some("Pick a generation model first".into()));
            return;
        };
        set_busy.set(true);
        set_build_error.set(None);
        spawn_local(async move {
            match request_document_map_build(RequestMapBuildRequestDto {
                document_id,
                document_version,
                generation_model_id: model_id,
            })
            .await
            {
                Ok(_) => {
                    set_busy.set(false);
                    map_resource.refetch();
                }
                Err(e) => {
                    set_busy.set(false);
                    set_build_error.set(Some(e.to_string()));
                }
            }
        });
    };

    let on_rebuild = move |_| {
        let Some(model_id) = selected_model.get() else {
            set_build_error.set(Some("Pick a generation model first".into()));
            return;
        };
        set_busy.set(true);
        set_build_error.set(None);
        spawn_local(async move {
            match rebuild_document_map(RequestMapBuildRequestDto {
                document_id,
                document_version,
                generation_model_id: model_id,
            })
            .await
            {
                Ok(_) => {
                    set_busy.set(false);
                    map_resource.refetch();
                }
                Err(e) => {
                    set_busy.set(false);
                    set_build_error.set(Some(e.to_string()));
                }
            }
        });
    };

    let eyebrow_label = format!("Documents / {source_ref_key} / Map");

    view! {
        <div>
            <Transition fallback=|| view! { <p class="muted">"Loading map…"</p> }>
                {
                    let title = document_title.clone();
                    let eyebrow = eyebrow_label.clone();
                    move || map_resource.get().map(|res| {
                        let title = title.clone();
                        let eyebrow = eyebrow.clone();
                        match res {
                            Err(e) => view! {
                                <div>
                                    <PageHeader
                                        title=title
                                        eyebrow=eyebrow
                                        subtitle=format!("Version {document_version}")
                                    />
                                    <Surface>
                                        <div class="log-line-error">{format!("Failed to load map: {e}")}</div>
                                    </Surface>
                                </div>
                            }.into_any(),
                            Ok(None) => view! {
                                <div>
                                    <PageHeader
                                        title=title
                                        eyebrow=eyebrow
                                        subtitle=format!("Version {document_version} · no map yet")
                                    />
                                    <MapMissingPanel
                                        configuration_loader=configuration
                                        selected_model=selected_model
                                        set_selected_model=set_selected_model
                                        busy=busy
                                        build_error=build_error
                                        on_build=on_build
                                    />
                                </div>
                            }.into_any(),
                            Ok(Some(detail)) => view! {
                                <MapDetailView
                                    detail=detail
                                    document_title=title
                                    eyebrow=eyebrow
                                    document_version=document_version
                                    configuration_loader=configuration
                                    selected_model=selected_model
                                    set_selected_model=set_selected_model
                                    busy=busy
                                    build_error=build_error
                                    on_rebuild=on_rebuild
                                />
                            }.into_any(),
                        }
                    })
                }
            </Transition>
        </div>
    }
}

#[component]
fn MapMissingPanel(
    configuration_loader: Resource<Result<ConfigurationDto, String>>,
    selected_model: ReadSignal<Option<Uuid>>,
    set_selected_model: WriteSignal<Option<Uuid>>,
    busy: ReadSignal<bool>,
    build_error: ReadSignal<Option<String>>,
    on_build: impl Fn(()) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <Surface>
            <EmptyState
                title="No map yet"
                body="A comprehension map has not been built for this document version. Building one extracts observations per chunk, synthesizes threads per section, then derives cross-cutting insights.".to_string()
            />
            <BuildForm
                configuration_loader=configuration_loader
                selected_model=selected_model
                set_selected_model=set_selected_model
                busy=busy
                build_error=build_error
                on_build=on_build
                primary_label="Build map"
                busy_label="Building…"
            />
        </Surface>
    }
}

#[component]
#[allow(clippy::too_many_arguments)]
fn MapDetailView(
    detail: DocumentMapDetailDto,
    document_title: String,
    eyebrow: String,
    document_version: u32,
    configuration_loader: Resource<Result<ConfigurationDto, String>>,
    selected_model: ReadSignal<Option<Uuid>>,
    set_selected_model: WriteSignal<Option<Uuid>>,
    busy: ReadSignal<bool>,
    build_error: ReadSignal<Option<String>>,
    on_rebuild: impl Fn(()) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let summary = detail.summary;
    let carried_summary = detail.carried_summary;
    let observations = detail.observations;
    let threads = detail.threads;
    let insights = detail.insights;

    let phase = MapPhase::from_status(&summary.status);
    let (status_kind, status_label_text) = phase.pill(MapProgress {
        observations_extracted: summary.observations_extracted,
        chunk_count: summary.chunk_count,
        threads_synthesized: summary.threads_synthesized,
        section_size: summary.section_size,
    });

    let chunk_count = summary.chunk_count;
    let section_total = section_count(summary.chunk_count, summary.section_size);
    let observations_extracted = summary.observations_extracted;
    let threads_synthesized = summary.threads_synthesized;
    let insights_synthesized = summary.insights_synthesized;
    let suggested_roles = summary.suggested_roles.clone();
    let failure_reason = summary.failure_reason.clone();

    let observation_count = observations.len();
    let thread_count = threads.len();
    let insight_count = insights.len();

    let initial = if insight_count > 0 {
        Tier::Insights
    } else if thread_count > 0 {
        Tier::Threads
    } else {
        Tier::Observations
    };
    let (active_tab, set_active_tab) = signal(initial);
    let (rebuild_open, set_rebuild_open) = signal(false);

    let observations_stored = StoredValue::new(observations);
    let threads_stored = StoredValue::new(threads);
    let insights_stored = StoredValue::new(insights);

    let subtitle_text =
        format!("v{document_version} · {chunk_count} chunks · {section_total} sections");
    let status_label_for_header = status_label_text.clone();

    let open_rebuild = Callback::new(move |_| set_rebuild_open.set(true));
    let close_rebuild = Callback::new(move |_| set_rebuild_open.set(false));
    let actions_items = vec![ActionItem::new("Rebuild map", open_rebuild).danger()];

    view! {
        <div>
            <PageHeader
                title=document_title
                eyebrow=eyebrow
                subtitle=subtitle_text
                actions=Box::new(move || view! {
                    <div class="flex items-center gap-2">
                        <StatusPill label=status_label_for_header.clone() kind=status_kind />
                        <ActionsMenu items=actions_items.clone() />
                    </div>
                }.into_any())
            />

            <RebuildDialog
                open=rebuild_open
                on_close=close_rebuild
                configuration_loader=configuration_loader
                selected_model=selected_model
                set_selected_model=set_selected_model
                busy=busy
                build_error=build_error
                on_rebuild=on_rebuild
            />

            <div class="space-y-4">
                {(!phase.is_ready() && !phase.is_failed()).then(|| view! {
                    <PipelineStrip
                        phase=phase
                        chunk_count=chunk_count
                        section_total=section_total
                        observations_extracted=observations_extracted
                        threads_synthesized=threads_synthesized
                        insights_synthesized=insights_synthesized
                    />
                })}

                {failure_reason.clone().map(|r| view! {
                    <Surface>
                        <div class="text-sm">
                            <div class="eyebrow mb-1">"Build failed"</div>
                            <div class="log-line-error">{r}</div>
                            <p class="muted mt-2 text-xs">
                                "Open the actions menu and choose Rebuild map to try again."
                            </p>
                        </div>
                    </Surface>
                })}

                {(!carried_summary.trim().is_empty() || !suggested_roles.is_empty()).then(|| view! {
                    <Surface>
                        <div class="flex flex-col gap-4">
                            {(!carried_summary.trim().is_empty()).then(|| {
                                let text = carried_summary.clone();
                                view! {
                                    <div>
                                        <div class="eyebrow mb-1">"Document summary"</div>
                                        <p class="text-sm leading-relaxed max-w-prose">{text}</p>
                                    </div>
                                }
                            })}
                            {(!suggested_roles.is_empty()).then(|| {
                                let roles = suggested_roles.clone();
                                view! { <RolesStrip roles=roles /> }
                            })}
                        </div>
                    </Surface>
                })}

                <TierTabs
                    active=active_tab
                    set_active=set_active_tab
                    insights=insight_count
                    threads=thread_count
                    observations=observation_count
                />

                {move || match active_tab.get() {
                    Tier::Insights => view! { <InsightsTab items=insights_stored /> }.into_any(),
                    Tier::Threads => view! {
                        <ThreadsTab items=threads_stored section_total=section_total />
                    }.into_any(),
                    Tier::Observations => view! { <ObservationsTab items=observations_stored /> }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
fn BuildForm(
    configuration_loader: Resource<Result<ConfigurationDto, String>>,
    selected_model: ReadSignal<Option<Uuid>>,
    set_selected_model: WriteSignal<Option<Uuid>>,
    busy: ReadSignal<bool>,
    build_error: ReadSignal<Option<String>>,
    on_build: impl Fn(()) + Copy + Send + Sync + 'static,
    #[prop(into)] primary_label: String,
    #[prop(into)] busy_label: String,
) -> impl IntoView {
    let primary_label = StoredValue::new(primary_label);
    let busy_label = StoredValue::new(busy_label);
    view! {
        <div class="mt-4 flex flex-col gap-3">
            <label class="eyebrow">"Generation model"</label>
            <Transition fallback=|| view! { <span class="muted text-sm">"Loading models…"</span> }>
                {move || configuration_loader.get().map(|res| match res {
                    Err(e) => Either::Left(view! {
                        <div class="log-line-error text-sm">{format!("Failed to load models: {e}")}</div>
                    }),
                    Ok(cfg) => {
                        let options: Vec<_> = cfg.generation_models.into_iter().collect();
                        Either::Right(view! {
                            <select
                                class="input max-w-md"
                                on:change=move |ev| {
                                    let id = Uuid::parse_str(&event_target_value(&ev)).ok();
                                    set_selected_model.set(id);
                                }
                            >
                                {options.into_iter().map(|model| {
                                    let is_selected = selected_model.get_untracked() == Some(model.generation_model_id);
                                    view! {
                                        <option value=model.generation_model_id.to_string() selected=is_selected>
                                            {format!("{} · {}", model.kind.as_str(), model.model)}
                                        </option>
                                    }
                                }).collect_view()}
                            </select>
                        })
                    }
                })}
            </Transition>
            {move || build_error.get().map(|e| view! {
                <div class="log-line-error text-sm">{e}</div>
            })}
            <button
                type="button"
                class="btn btn-primary self-start"
                disabled=move || busy.get()
                on:click=move |_| on_build(())
            >
                {move || if busy.get() {
                    busy_label.get_value()
                } else {
                    primary_label.get_value()
                }}
            </button>
        </div>
    }
}

#[component]
fn RebuildDialog(
    open: ReadSignal<bool>,
    on_close: Callback<()>,
    configuration_loader: Resource<Result<ConfigurationDto, String>>,
    selected_model: ReadSignal<Option<Uuid>>,
    set_selected_model: WriteSignal<Option<Uuid>>,
    busy: ReadSignal<bool>,
    build_error: ReadSignal<Option<String>>,
    on_rebuild: impl Fn(()) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let on_confirm = move |_| {
        on_rebuild(());
        on_close.run(());
    };
    view! {
        <Dialog
            open=Signal::derive(move || open.get())
            title="Rebuild map".to_string()
            subtitle="Replaces the current comprehension map for this document version.".to_string()
            on_close=on_close
        >
            <div class="space-y-4">
                <p class="text-sm muted">
                    "All observations, threads, and insights for this version will be discarded and re-extracted. The new build will run from scratch with the selected generation model."
                </p>
                <div class="flex flex-col gap-2">
                    <label class="eyebrow">"Generation model"</label>
                    <Transition fallback=|| view! { <span class="muted text-sm">"Loading models…"</span> }>
                        {move || configuration_loader.get().map(|res| match res {
                            Err(e) => Either::Left(view! {
                                <div class="log-line-error text-sm">{format!("Failed to load models: {e}")}</div>
                            }),
                            Ok(cfg) => {
                                let options: Vec<_> = cfg.generation_models.into_iter().collect();
                                Either::Right(view! {
                                    <select
                                        class="input"
                                        on:change=move |ev| {
                                            let id = Uuid::parse_str(&event_target_value(&ev)).ok();
                                            set_selected_model.set(id);
                                        }
                                    >
                                        {options.into_iter().map(|model| {
                                            let is_selected = selected_model.get_untracked() == Some(model.generation_model_id);
                                            view! {
                                                <option value=model.generation_model_id.to_string() selected=is_selected>
                                                    {format!("{} · {}", model.kind.as_str(), model.model)}
                                                </option>
                                            }
                                        }).collect_view()}
                                    </select>
                                })
                            }
                        })}
                    </Transition>
                </div>
                {move || build_error.get().map(|e| view! {
                    <div class="log-line-error text-sm">{e}</div>
                })}
                <div class="flex justify-end gap-2 pt-2 border-t border-[var(--color-border)]">
                    <button
                        type="button"
                        class="btn"
                        disabled=move || busy.get()
                        on:click=move |_| on_close.run(())
                    >
                        "Cancel"
                    </button>
                    <button
                        type="button"
                        class="btn btn-danger"
                        disabled=move || busy.get() || selected_model.get().is_none()
                        on:click=on_confirm
                    >
                        {move || if busy.get() { "Rebuilding…" } else { "Rebuild map" }}
                    </button>
                </div>
            </div>
        </Dialog>
    }
}

#[component]
fn PipelineStrip(
    phase: MapPhase,
    chunk_count: u32,
    section_total: u32,
    observations_extracted: u32,
    threads_synthesized: u32,
    insights_synthesized: u32,
) -> impl IntoView {
    let stages = [
        Stage {
            label: "Roles",
            state: phase_stage_state(phase, MapPhase::TypingRoles),
            detail: String::new(),
        },
        Stage {
            label: "Observations",
            state: phase_stage_state(phase, MapPhase::ExtractingObservations),
            detail: format!("{observations_extracted}/{chunk_count}"),
        },
        Stage {
            label: "Threads",
            state: phase_stage_state(phase, MapPhase::SynthesizingThreads),
            detail: format!("{threads_synthesized}/{section_total}"),
        },
        Stage {
            label: "Insights",
            state: phase_stage_state(phase, MapPhase::SynthesizingInsights),
            detail: format!("{insights_synthesized}"),
        },
    ];

    view! {
        <Surface>
            <div class="eyebrow mb-3">"Building map"</div>
            <div class="flex items-stretch gap-2 flex-wrap">
                {stages.into_iter().enumerate().map(|(idx, s)| {
                    let is_last = idx == 3;
                    let (icon, color_class, label_class) = match s.state {
                        StageState::Done => ("✓", "bg-[var(--color-accent-soft)] text-[var(--color-accent)] border-[var(--color-accent)]", "text-[var(--color-text)]"),
                        StageState::Active => ("●", "bg-[var(--status-info)] text-[var(--color-page-bg)] border-[var(--status-info)] animate-pulse", "text-[var(--color-text)]"),
                        StageState::Pending => ("○", "bg-transparent text-[var(--color-text-faint)] border-[var(--color-border)]", "text-[var(--color-text-muted)]"),
                    };
                    view! {
                        <div class="flex items-center gap-2 min-w-0">
                            <div class="flex items-center gap-2">
                                <span class=format!("inline-flex items-center justify-center w-6 h-6 rounded-full border text-xs {color_class}")>
                                    {icon}
                                </span>
                                <div class="flex flex-col leading-tight">
                                    <span class=format!("text-xs font-semibold {label_class}")>{s.label}</span>
                                    {(!s.detail.is_empty()).then(|| view! {
                                        <span class="text-[11px] muted font-mono">{s.detail}</span>
                                    })}
                                </div>
                            </div>
                            {(!is_last).then(|| view! { <span class="faint px-1">"›"</span> })}
                        </div>
                    }
                }).collect_view()}
            </div>
        </Surface>
    }
}

#[component]
fn RolesStrip(roles: Vec<SuggestedRoleDto>) -> impl IntoView {
    view! {
        <div>
            <div class="eyebrow mb-2">"Suggested perspectives"</div>
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
                {roles.into_iter().map(|r| view! {
                    <div class="surface-raised rounded px-3 py-2 flex flex-col gap-0.5 min-w-0">
                        <div class="text-sm font-semibold text-[var(--color-text)] truncate">
                            {r.name}
                        </div>
                        <div class="text-xs muted leading-snug break-words">
                            {r.focus}
                        </div>
                    </div>
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn TierTabs(
    active: ReadSignal<Tier>,
    set_active: WriteSignal<Tier>,
    insights: usize,
    threads: usize,
    observations: usize,
) -> impl IntoView {
    let tabs = [
        (Tier::Insights, "Insights", insights),
        (Tier::Threads, "Threads", threads),
        (Tier::Observations, "Observations", observations),
    ];
    view! {
        <div class="seg-toggle" role="tablist" aria-label="Comprehension tier">
            {tabs.into_iter().map(|(tier, label, count)| {
                view! {
                    <button
                        type="button"
                        role="tab"
                        class:is-active=move || active.get() == tier
                        aria-selected=move || (active.get() == tier).to_string()
                        on:click=move |_| set_active.set(tier)
                    >
                        <span>{label}</span>
                        <span class="ml-1.5 list-page-chip-count">{count.to_string()}</span>
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn InsightsTab(items: StoredValue<Vec<InsightDto>>) -> impl IntoView {
    let kinds = items.with_value(|v| kind_counts(v.iter().map(|i| i.kind.as_str())));
    let (kind_filter, set_kind_filter) = signal::<Option<String>>(None);

    let filtered = move || {
        items.with_value(|v| {
            let filter = kind_filter.get();
            v.iter()
                .filter(|i| filter.as_deref().is_none_or(|k| k == i.kind))
                .cloned()
                .collect::<Vec<_>>()
        })
    };

    view! {
        <Surface>
            {if items.with_value(Vec::is_empty) {
                Either::Left(view! {
                    <EmptyState
                        title="No insights yet"
                        body="Insights are the top-tier synthesis of the document. They appear once thread synthesis completes.".to_string()
                    />
                })
            } else {
                Either::Right(view! {
                    <div class="space-y-3">
                        <KindFilterRow kinds=kinds active=kind_filter set_active=set_kind_filter />
                        <div class="space-y-2">
                            {move || filtered().into_iter().map(|i| view! {
                                <InsightCard insight=i />
                            }).collect_view()}
                        </div>
                    </div>
                })
            }}
        </Surface>
    }
}

#[component]
fn ThreadsTab(items: StoredValue<Vec<ThreadDto>>, section_total: u32) -> impl IntoView {
    let kinds = items.with_value(|v| kind_counts(v.iter().map(|i| i.kind.as_str())));
    let (kind_filter, set_kind_filter) = signal::<Option<String>>(None);
    let show_section = section_total > 1;

    let filtered = move || {
        items.with_value(|v| {
            let filter = kind_filter.get();
            let mut out: Vec<ThreadDto> = v
                .iter()
                .filter(|t| filter.as_deref().is_none_or(|k| k == t.kind))
                .cloned()
                .collect();
            out.sort_by_key(|t| t.section_sequence);
            out
        })
    };

    view! {
        <Surface>
            {if items.with_value(Vec::is_empty) {
                Either::Left(view! {
                    <EmptyState
                        title="No threads yet"
                        body="Threads synthesize observations within a section. They appear once the section's observations have been extracted.".to_string()
                    />
                })
            } else {
                Either::Right(view! {
                    <div class="space-y-3">
                        <KindFilterRow kinds=kinds active=kind_filter set_active=set_kind_filter />
                        <div class="space-y-2">
                            {move || filtered().into_iter().map(|t| view! {
                                <ThreadCard thread=t show_section=show_section />
                            }).collect_view()}
                        </div>
                    </div>
                })
            }}
        </Surface>
    }
}

#[component]
fn ObservationsTab(items: StoredValue<Vec<ObservationDto>>) -> impl IntoView {
    let kinds = items.with_value(|v| kind_counts(v.iter().map(|i| i.kind.as_str())));
    let (kind_filter, set_kind_filter) = signal::<Option<String>>(None);

    let filtered = move || {
        items.with_value(|v| {
            let filter = kind_filter.get();
            let mut out: Vec<ObservationDto> = v
                .iter()
                .filter(|o| filter.as_deref().is_none_or(|k| k == o.kind))
                .cloned()
                .collect();
            out.sort_by_key(|o| o.chunk_sequence);
            out
        })
    };

    view! {
        <Surface>
            {if items.with_value(Vec::is_empty) {
                Either::Left(view! {
                    <EmptyState
                        title="No observations yet"
                        body="Observations are raw extractions from each chunk. They populate as the build progresses.".to_string()
                    />
                })
            } else {
                Either::Right(view! {
                    <div class="space-y-3">
                        <KindFilterRow kinds=kinds active=kind_filter set_active=set_kind_filter />
                        <div class="space-y-2">
                            {move || filtered().into_iter().map(|o| view! {
                                <ObservationCard observation=o />
                            }).collect_view()}
                        </div>
                    </div>
                })
            }}
        </Surface>
    }
}

#[component]
fn KindFilterRow(
    kinds: Vec<(String, usize)>,
    active: ReadSignal<Option<String>>,
    set_active: WriteSignal<Option<String>>,
) -> impl IntoView {
    if kinds.is_empty() {
        return view! { <div></div> }.into_any();
    }
    let total: usize = kinds.iter().map(|(_, c)| c).sum();
    view! {
        <div class="flex flex-wrap items-center gap-2">
            <span class="eyebrow">"Kind"</span>
            <button
                type="button"
                class="list-page-chip"
                data-active=move || active.get().is_none().to_string()
                on:click=move |_| set_active.set(None)
            >
                <span>"All"</span>
                <span class="list-page-chip-count">{total.to_string()}</span>
            </button>
            {kinds.into_iter().map(|(k, c)| {
                let key = k.clone();
                let key_for_active = k.clone();
                let key_for_click = k.clone();
                view! {
                    <button
                        type="button"
                        class="list-page-chip"
                        data-active=move || (active.get().as_deref() == Some(&key_for_active)).to_string()
                        on:click=move |_| set_active.set(Some(key_for_click.clone()))
                    >
                        <span>{key}</span>
                        <span class="list-page-chip-count">{c.to_string()}</span>
                    </button>
                }
            }).collect_view()}
        </div>
    }.into_any()
}

const KIND_PILL_CLASS: &str = "pill pill-info shrink-0 justify-center min-w-[7.5rem]";
const ROW_BADGE_CLASS: &str = "font-mono text-[11px] muted shrink-0 w-20";

#[component]
fn InsightCard(insight: InsightDto) -> impl IntoView {
    let evidence_summary = summarize_evidence(&insight.evidence);
    let spans_label = format!(
        "{} span{}",
        insight.spans.len(),
        plural(insight.spans.len())
    );
    view! {
        <details class="surface-raised rounded p-3">
            <summary class="cursor-pointer flex items-start gap-3 list-none">
                <span class=KIND_PILL_CLASS>{insight.kind.clone()}</span>
                <span class="flex-1 text-sm leading-snug break-words">{insight.summary.clone()}</span>
                <span class="text-[11px] muted font-mono shrink-0 whitespace-nowrap">
                    {format!("{spans_label} · {evidence_summary}")}
                </span>
            </summary>
            <div class="mt-3 pt-3 border-t border-[var(--color-border)] grid grid-cols-1 md:grid-cols-2 gap-4">
                <SpansList spans=insight.spans.clone() />
                <EvidenceList evidence=insight.evidence.clone() />
            </div>
        </details>
    }
}

#[component]
fn ThreadCard(thread: ThreadDto, show_section: bool) -> impl IntoView {
    let evidence_summary = summarize_evidence(&thread.evidence);
    let spans_label = format!("{} span{}", thread.spans.len(), plural(thread.spans.len()));
    let section = thread.section_sequence;
    view! {
        <details class="surface-raised rounded p-3">
            <summary class="cursor-pointer flex items-start gap-3 list-none">
                {show_section.then(|| view! {
                    <span class=ROW_BADGE_CLASS>{format!("section {section}")}</span>
                })}
                <span class=KIND_PILL_CLASS>{thread.kind.clone()}</span>
                <span class="flex-1 text-sm leading-snug break-words">{thread.summary.clone()}</span>
                <span class="text-[11px] muted font-mono shrink-0 whitespace-nowrap">
                    {format!("{spans_label} · {evidence_summary}")}
                </span>
            </summary>
            <div class="mt-3 pt-3 border-t border-[var(--color-border)] grid grid-cols-1 md:grid-cols-2 gap-4">
                <SpansList spans=thread.spans.clone() />
                <EvidenceList evidence=thread.evidence.clone() />
            </div>
        </details>
    }
}

#[component]
fn ObservationCard(observation: ObservationDto) -> impl IntoView {
    let spans_label = format!(
        "{} span{}",
        observation.spans.len(),
        plural(observation.spans.len())
    );
    let chunk = observation.chunk_sequence;
    let has_spans = !observation.spans.is_empty();
    view! {
        <details class="surface-raised rounded p-3">
            <summary class="cursor-pointer flex items-start gap-3 list-none">
                <span class=ROW_BADGE_CLASS>{format!("chunk {chunk:03}")}</span>
                <span class=KIND_PILL_CLASS>{observation.kind.clone()}</span>
                <span class="flex-1 text-sm leading-snug break-words">{observation.summary.clone()}</span>
                <span class="text-[11px] muted font-mono shrink-0 whitespace-nowrap">{spans_label}</span>
            </summary>
            {has_spans.then(|| view! {
                <div class="mt-3 pt-3 border-t border-[var(--color-border)]">
                    <SpansList spans=observation.spans.clone() />
                </div>
            })}
        </details>
    }
}

#[component]
fn SpansList(spans: Vec<SpanDto>) -> impl IntoView {
    if spans.is_empty() {
        return view! {
            <div>
                <div class="eyebrow mb-1">"Spans"</div>
                <div class="text-xs muted">"None"</div>
            </div>
        }
        .into_any();
    }
    view! {
        <div>
            <div class="eyebrow mb-1">{format!("Spans ({})", spans.len())}</div>
            <ul class="font-mono text-[11px] space-y-0.5">
                {spans.into_iter().map(|s| view! {
                    <li class="muted">{format!("{}–{}", s.char_start, s.char_end)}</li>
                }).collect_view()}
            </ul>
        </div>
    }
    .into_any()
}

#[component]
fn EvidenceList(evidence: Vec<MapItemRefDto>) -> impl IntoView {
    if evidence.is_empty() {
        return view! {
            <div>
                <div class="eyebrow mb-1">"Evidence"</div>
                <div class="text-xs muted">"None"</div>
            </div>
        }
        .into_any();
    }
    let mut observations = 0_usize;
    let mut threads = 0_usize;
    let mut insights = 0_usize;
    let mut connections = 0_usize;
    for r in &evidence {
        match r {
            MapItemRefDto::Observation { .. } => observations += 1,
            MapItemRefDto::Thread { .. } => threads += 1,
            MapItemRefDto::Insight { .. } => insights += 1,
            MapItemRefDto::Connection { .. } => connections += 1,
        }
    }
    let total = evidence.len();
    view! {
        <div>
            <div class="eyebrow mb-1">{format!("Evidence ({total})")}</div>
            <div class="flex flex-wrap gap-1.5 text-[11px]">
                {(observations > 0).then(|| view! {
                    <span class="pill pill-neutral">{format!("{observations} observations")}</span>
                })}
                {(threads > 0).then(|| view! {
                    <span class="pill pill-neutral">{format!("{threads} threads")}</span>
                })}
                {(insights > 0).then(|| view! {
                    <span class="pill pill-neutral">{format!("{insights} insights")}</span>
                })}
                {(connections > 0).then(|| view! {
                    <span class="pill pill-neutral">{format!("{connections} connections")}</span>
                })}
            </div>
        </div>
    }
    .into_any()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    Insights,
    Threads,
    Observations,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StageState {
    Pending,
    Active,
    Done,
}

struct Stage {
    label: &'static str,
    state: StageState,
    detail: String,
}

fn phase_stage_state(phase: MapPhase, stage: MapPhase) -> StageState {
    use std::cmp::Ordering;
    if phase.is_failed() {
        return StageState::Pending;
    }
    match phase.order().cmp(&stage.order()) {
        Ordering::Greater => StageState::Done,
        Ordering::Equal => StageState::Active,
        Ordering::Less => StageState::Pending,
    }
}

fn kind_counts<'a>(iter: impl Iterator<Item = &'a str>) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for k in iter {
        *counts.entry(k.to_string()).or_default() += 1;
    }
    counts.into_iter().collect()
}

fn summarize_evidence(evidence: &[MapItemRefDto]) -> String {
    let n = evidence.len();
    if n == 0 {
        "no evidence".to_string()
    } else {
        format!("{n} evidence")
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
