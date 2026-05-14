use leptos::prelude::*;
use uuid::Uuid;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{Dialog, EmptyState, PageHeader, Surface};
use crate::pages::configuration::commands::{
    parse_uuid_or_none, run_chunking_configuration_command, run_sweep_template_command,
};
use crate::server_functions::configuration::{
    get_chunking_configurations, get_configuration, get_sweep_templates,
};
use crate::shared::{
    aggregate_type, BertChunkingConfig, ChunkStrategy, ChunkingConfig,
    ChunkingConfigurationCommandDto, ChunkingConfigurationDto, ConfigurationDto,
    CreateChunkingConfigurationDto, CreateSweepTemplateDto, DeleteChunkingConfigurationDto,
    DeleteSweepTemplateDto, LlmChunkingConfig, SectionChunkingConfig, SetDefaultSweepTemplateDto,
    SweepTemplateCommandDto, SweepTemplateDto, UpdateChunkingConfigurationDto,
    UpdateSweepTemplateDto,
};

#[derive(Clone)]
enum FormMode {
    Add,
    Edit(ChunkingConfigurationDto),
}

#[derive(Clone)]
enum SweepFormMode {
    Add,
    Edit(SweepTemplateDto),
}

#[component]
pub fn ChunkingPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::GENERATION_MODEL_CATALOG,
            aggregate_type::SWEEP_TEMPLATE,
        ])
    });
    let (refresh, set_refresh) = signal(0u32);

    let configurations = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move {
            get_chunking_configurations()
                .await
                .map_err(|e| e.to_string())
        },
    );
    let configuration = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_configuration().await.map_err(|e| e.to_string()) },
    );
    let sweep_templates = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_sweep_templates().await.map_err(|e| e.to_string()) },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<(bool, String)>>(None);
    let (form_mode, set_form_mode) = signal::<Option<FormMode>>(None);
    let (delete_target, set_delete_target) = signal::<Option<ChunkingConfigurationDto>>(None);
    let (sweep_form_mode, set_sweep_form_mode) = signal::<Option<SweepFormMode>>(None);
    let (sweep_delete_target, set_sweep_delete_target) = signal::<Option<SweepTemplateDto>>(None);

    view! {
        <div>
            <PageHeader
                title="Chunking"
                subtitle="Named chunking configurations — pick a strategy, tune the parameters, and reuse them across ingestions and evaluations.".to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=move |_| set_form_mode.set(Some(FormMode::Add))
                    >
                        "+ New chunking config"
                    </button>
                }.into_any())
            />

            <StatusBanner status=status />

            <Transition fallback=|| view! { <p class="muted">"Loading chunking configurations…"</p> }>
                {move || {
                    let list = match configurations.get() {
                        Some(Ok(l)) => l,
                        Some(Err(e)) => {
                            return view! {
                                <Surface>
                                    <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                                </Surface>
                            }.into_any();
                        }
                        None => return ().into_any(),
                    };

                    view! {
                        <ChunkingList
                            configurations=list
                            on_edit=Callback::new(move |cc: ChunkingConfigurationDto| set_form_mode.set(Some(FormMode::Edit(cc))))
                            on_delete=Callback::new(move |cc: ChunkingConfigurationDto| set_delete_target.set(Some(cc)))
                            busy=busy
                        />
                    }.into_any()
                }}
            </Transition>

            {move || configuration.get().map(|res| match res {
                Ok(cfg) => view! {
                    <ChunkingFormDialog
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

            <DeleteConfirmDialog
                target=delete_target
                set_target=set_delete_target
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />

            <div class="mt-8 mb-3 flex items-end justify-between gap-3">
                <div>
                    <h2 class="section-title">"Sweep templates"</h2>
                    <p class="text-sm muted">
                        "Named bundles of chunking configurations. Pick one as the variants axis when launching an autotune."
                    </p>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    on:click=move |_| set_sweep_form_mode.set(Some(SweepFormMode::Add))
                >
                    "+ New sweep template"
                </button>
            </div>

            <Transition fallback=|| view! { <p class="muted">"Loading sweep templates…"</p> }>
                {move || {
                    let templates = match sweep_templates.get() {
                        Some(Ok(t)) => t,
                        Some(Err(e)) => {
                            return view! {
                                <Surface>
                                    <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                                </Surface>
                            }.into_any();
                        }
                        None => return ().into_any(),
                    };
                    let chunking_configs = configurations.get().and_then(|r| r.ok()).unwrap_or_default();

                    view! {
                        <SweepTemplateList
                            templates=templates
                            chunking_configs=chunking_configs
                            on_edit=Callback::new(move |st: SweepTemplateDto| set_sweep_form_mode.set(Some(SweepFormMode::Edit(st))))
                            on_delete=Callback::new(move |st: SweepTemplateDto| set_sweep_delete_target.set(Some(st)))
                            on_set_default=Callback::new(move |st: SweepTemplateDto| {
                                run_sweep_template_command(
                                    SweepTemplateCommandDto::SetDefaultSweepTemplate(SetDefaultSweepTemplateDto {
                                        sweep_template_id: st.sweep_template_id,
                                    }),
                                    "Default sweep template set",
                                    set_busy,
                                    set_status,
                                    None,
                                    set_refresh,
                                    || {},
                                );
                            })
                            on_duplicate=Callback::new(move |st: SweepTemplateDto| {
                                run_sweep_template_command(
                                    SweepTemplateCommandDto::CreateSweepTemplate(CreateSweepTemplateDto {
                                        name: format!("{}-copy", st.name),
                                        members: st.members,
                                    }),
                                    "Sweep template duplicated",
                                    set_busy,
                                    set_status,
                                    None,
                                    set_refresh,
                                    || {},
                                );
                            })
                            busy=busy
                        />
                    }.into_any()
                }}
            </Transition>

            <SweepTemplateFormDialog
                form_mode=sweep_form_mode
                set_form_mode=set_sweep_form_mode
                chunking_configs_resource=configurations
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />

            <SweepTemplateDeleteDialog
                target=sweep_delete_target
                set_target=set_sweep_delete_target
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
fn ChunkingList(
    configurations: Vec<ChunkingConfigurationDto>,
    on_edit: Callback<ChunkingConfigurationDto>,
    on_delete: Callback<ChunkingConfigurationDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    if configurations.is_empty() {
        return view! {
            <Surface>
                <EmptyState
                    title="No chunking configurations yet"
                    body="A chunking configuration bundles a strategy (section, bert, llm) with its tunables. Create one and reuse it across ingestions and evaluation sweeps.".to_string()
                />
            </Surface>
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            {configurations.into_iter().map(|cc| view! {
                <ChunkingCard cc=cc on_edit=on_edit on_delete=on_delete busy=busy />
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn ChunkingCard(
    cc: ChunkingConfigurationDto,
    on_edit: Callback<ChunkingConfigurationDto>,
    on_delete: Callback<ChunkingConfigurationDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let cc_edit = cc.clone();
    let cc_delete = cc.clone();
    let name = cc.name.clone();
    let strategy_id = cc.config.strategy().as_str();
    let descriptor = cc.config.describe();

    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <h3 class="section-title">{name}</h3>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{format!("strategy · {strategy_id}")}</span>
                    <span class="pill pill-neutral">{descriptor}</span>
                </div>
            </div>
            <div class="flex gap-2 shrink-0">
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_edit.run(cc_edit.clone())
                >
                    "Edit"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_delete.run(cc_delete.clone())
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}

#[component]
fn ChunkingFormDialog(
    config: ConfigurationDto,
    form_mode: ReadSignal<Option<FormMode>>,
    set_form_mode: WriteSignal<Option<FormMode>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let config = StoredValue::new(config);
    let default_llm = move || {
        let mut llm = LlmChunkingConfig::default();
        if let Some(first) = config.with_value(|c| c.generation_models.first().cloned()) {
            llm.generation_model_id = first.generation_model_id;
        }
        llm
    };

    let (name, set_name) = signal(String::new());
    let (strategy, set_strategy) = signal(ChunkStrategy::default());
    let (section_cfg, set_section_cfg) = signal(SectionChunkingConfig::default());
    let (bert_cfg, set_bert_cfg) = signal(BertChunkingConfig::default());
    let (llm_cfg, set_llm_cfg) = signal(default_llm());
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form_mode.get() {
            None => {}
            Some(FormMode::Add) => {
                set_name.set(String::new());
                set_strategy.set(ChunkStrategy::default());
                set_section_cfg.set(SectionChunkingConfig::default());
                set_bert_cfg.set(BertChunkingConfig::default());
                set_llm_cfg.set(default_llm());
            }
            Some(FormMode::Edit(cc)) => {
                set_name.set(cc.name);
                set_strategy.set(cc.config.strategy());
                match cc.config {
                    ChunkingConfig::Section(c) => set_section_cfg.set(c),
                    ChunkingConfig::Bert(c) => set_bert_cfg.set(c),
                    ChunkingConfig::Llm(c) => set_llm_cfg.set(c),
                }
            }
        }
    });

    let current_config = move || match strategy.get() {
        ChunkStrategy::Section => ChunkingConfig::Section(section_cfg.get()),
        ChunkStrategy::Bert => ChunkingConfig::Bert(bert_cfg.get()),
        ChunkStrategy::Llm => ChunkingConfig::Llm(llm_cfg.get()),
    };

    let close = Callback::new(move |_| {
        set_form_mode.set(None);
        set_dialog_error.set(None);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name_val = name.get().trim().to_string();
        if name_val.is_empty() {
            set_dialog_error.set(Some("Name is required.".into()));
            return;
        }
        let config = current_config();
        if let ChunkingConfig::Llm(llm) = &config {
            if llm.generation_model_id == Uuid::nil() {
                set_dialog_error.set(Some("Pick a generation model for LLM chunking.".into()));
                return;
            }
        }
        let command = match form_mode.get() {
            Some(FormMode::Add) => ChunkingConfigurationCommandDto::CreateChunkingConfiguration(
                CreateChunkingConfigurationDto {
                    name: name_val,
                    config,
                },
            ),
            Some(FormMode::Edit(cc)) => {
                ChunkingConfigurationCommandDto::UpdateChunkingConfiguration(
                    UpdateChunkingConfigurationDto {
                        chunking_configuration_id: cc.chunking_configuration_id,
                        name: name_val,
                        config,
                    },
                )
            }
            None => return,
        };
        run_chunking_configuration_command(
            command,
            "Chunking configuration saved",
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
                Some(FormMode::Edit(_)) => "Edit chunking configuration".to_string(),
                _ => "New chunking configuration".to_string(),
            }).get()
            subtitle="Pick a strategy and tune its parameters. Used at ingest time and as the Variants axis in evaluation sweeps.".to_string()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|msg| view! {
                    <div class="log-line-error text-sm">{msg}</div>
                })}

                <LabelledInput
                    label="Name".to_string()
                    hint="e.g. section-480, bert-target-384".to_string()
                    value=name
                    set_value=set_name
                />

                <StrategyTabs strategy=strategy set_strategy=set_strategy />

                <ParamFields
                    strategy=strategy
                    section_cfg=section_cfg
                    set_section_cfg=set_section_cfg
                    bert_cfg=bert_cfg
                    set_bert_cfg=set_bert_cfg
                    llm_cfg=llm_cfg
                    set_llm_cfg=set_llm_cfg
                    generation_models=Memo::new(move |_| config.with_value(|c| {
                        c.generation_models
                            .iter()
                            .map(|m| (m.generation_model_id, m.model.clone()))
                            .collect::<Vec<_>>()
                    }))
                />

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
fn StrategyTabs(
    strategy: ReadSignal<ChunkStrategy>,
    set_strategy: WriteSignal<ChunkStrategy>,
) -> impl IntoView {
    view! {
        <div class="space-y-1.5">
            <span class="eyebrow">"Strategy"</span>
            <div class="flex gap-2 flex-wrap">
                {ChunkStrategy::all().iter().map(|def| {
                    let s = def.strategy;
                    let label = def.label;
                    let hint = def.hint;
                    view! {
                        <button
                            type="button"
                            class=move || {
                                let base = "btn";
                                if strategy.get() == s { format!("{base} btn-primary") }
                                else { base.to_string() }
                            }
                            title=hint
                            on:click=move |_| set_strategy.set(s)
                        >
                            {label}
                        </button>
                    }
                }).collect_view()}
            </div>
            <p class="text-xs faint">
                {move || strategy.get().definition().hint}
            </p>
        </div>
    }
}

#[component]
fn ParamFields(
    strategy: ReadSignal<ChunkStrategy>,
    section_cfg: ReadSignal<SectionChunkingConfig>,
    set_section_cfg: WriteSignal<SectionChunkingConfig>,
    bert_cfg: ReadSignal<BertChunkingConfig>,
    set_bert_cfg: WriteSignal<BertChunkingConfig>,
    llm_cfg: ReadSignal<LlmChunkingConfig>,
    set_llm_cfg: WriteSignal<LlmChunkingConfig>,
    generation_models: Memo<Vec<(Uuid, String)>>,
) -> impl IntoView {
    view! {
        {move || match strategy.get() {
            ChunkStrategy::Section => view! {
                <NumberField
                    label="MAX_SECTION_TOKENS".to_string()
                    hint="section: max tokens per chunk before fallback split".to_string()
                    min=1
                    value=Signal::derive(move || section_cfg.get().max_section_tokens)
                    on_change=Callback::new(move |v: u32| {
                        set_section_cfg.update(|c| c.max_section_tokens = v);
                    })
                />
            }.into_any(),
            ChunkStrategy::Bert => view! {
                <div class="space-y-3">
                    <NumberField
                        label="TARGET_TOKENS".to_string()
                        hint="bert: target chunk size in tokens".to_string()
                        min=1
                        value=Signal::derive(move || bert_cfg.get().target_tokens)
                        on_change=Callback::new(move |v: u32| {
                            set_bert_cfg.update(|c| c.target_tokens = v);
                        })
                    />
                    <NumberField
                        label="OVERLAP_TOKENS".to_string()
                        hint="bert: tokens of overlap between adjacent chunks".to_string()
                        min=0
                        value=Signal::derive(move || bert_cfg.get().overlap_tokens)
                        on_change=Callback::new(move |v: u32| {
                            set_bert_cfg.update(|c| c.overlap_tokens = v);
                        })
                    />
                    <NumberField
                        label="MIN_TOKENS".to_string()
                        hint="bert: small trailing chunks merge with the previous one".to_string()
                        min=0
                        value=Signal::derive(move || bert_cfg.get().min_tokens)
                        on_change=Callback::new(move |v: u32| {
                            set_bert_cfg.update(|c| c.min_tokens = v);
                        })
                    />
                </div>
            }.into_any(),
            ChunkStrategy::Llm => view! {
                <div class="space-y-3">
                    <NumberField
                        label="TARGET_TOKENS".to_string()
                        hint="llm: maximum final chunk size in tokens".to_string()
                        min=1
                        value=Signal::derive(move || llm_cfg.get().target_tokens)
                        on_change=Callback::new(move |v: u32| {
                            set_llm_cfg.update(|c| c.target_tokens = v);
                        })
                    />
                    <NumberField
                        label="MICRO_CHUNK_TOKENS".to_string()
                        hint="llm: punctuation-aware micro chunks offered to the model for boundary selection".to_string()
                        min=32
                        value=Signal::derive(move || llm_cfg.get().micro_chunk_tokens)
                        on_change=Callback::new(move |v: u32| {
                            set_llm_cfg.update(|c| c.micro_chunk_tokens = v);
                        })
                    />
                    <label class="block space-y-1.5">
                        <span class="eyebrow">"GENERATION_MODEL"</span>
                        <select
                            class="input"
                            on:change=move |e| {
                                if let Some(id) = parse_uuid_or_none(&event_target_value(&e)) {
                                    set_llm_cfg.update(|c| c.generation_model_id = id);
                                }
                            }
                        >
                            <option value="">"— select generation model —"</option>
                            {move || generation_models.get().into_iter().map(|(id, label)| {
                                let selected = llm_cfg.get().generation_model_id == id;
                                view! { <option value=id.to_string() selected=selected>{label}</option> }
                            }).collect_view()}
                        </select>
                        <span class="text-xs faint">"Registry-backed. Used to select chunk boundaries over micro-chunks."</span>
                    </label>
                </div>
            }.into_any(),
        }}
    }
}

#[component]
fn DeleteConfirmDialog(
    target: ReadSignal<Option<ChunkingConfigurationDto>>,
    set_target: WriteSignal<Option<ChunkingConfigurationDto>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(cc) = target.get_untracked() else {
            return;
        };
        run_chunking_configuration_command(
            ChunkingConfigurationCommandDto::DeleteChunkingConfiguration(
                DeleteChunkingConfigurationDto {
                    chunking_configuration_id: cc.chunking_configuration_id,
                },
            ),
            "Chunking configuration deleted",
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
            title="Delete chunking configuration".to_string()
            subtitle="Existing indexings and runs already reference this by name. They keep working; new ones won't be able to pick it.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised p-3 rounded">
                    <span class="muted text-sm">"Configuration"</span>
                    <div class="text-text">{move || target.get().map(|cc| cc.name).unwrap_or_default()}</div>
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
fn NumberField(
    label: String,
    hint: String,
    min: u32,
    #[prop(into)] value: Signal<u32>,
    on_change: Callback<u32>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                type="number"
                min=min.to_string()
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let raw = event_target_value(&e);
                    if let Ok(parsed) = raw.parse::<u32>() {
                        let clamped = parsed.max(min);
                        on_change.run(clamped);
                    }
                }
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

#[component]
fn SweepTemplateList(
    templates: Vec<SweepTemplateDto>,
    chunking_configs: Vec<ChunkingConfigurationDto>,
    on_edit: Callback<SweepTemplateDto>,
    on_delete: Callback<SweepTemplateDto>,
    on_set_default: Callback<SweepTemplateDto>,
    on_duplicate: Callback<SweepTemplateDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    if templates.is_empty() {
        return view! {
            <Surface>
                <EmptyState
                    title="No sweep templates yet"
                    body="A sweep template names a bundle of chunking configurations. Pick it when launching an autotune to test those configurations side-by-side.".to_string()
                />
            </Surface>
        }
        .into_any();
    }

    let lookup: std::collections::HashMap<Uuid, String> = chunking_configs
        .into_iter()
        .map(|cc| (cc.chunking_configuration_id, cc.name))
        .collect();
    let lookup = StoredValue::new(lookup);

    view! {
        <div class="space-y-3">
            {templates.into_iter().map(|st| view! {
                <SweepTemplateCard
                    template=st
                    lookup=lookup
                    on_edit=on_edit
                    on_delete=on_delete
                    on_set_default=on_set_default
                    on_duplicate=on_duplicate
                    busy=busy
                />
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn SweepTemplateCard(
    template: SweepTemplateDto,
    lookup: StoredValue<std::collections::HashMap<Uuid, String>>,
    on_edit: Callback<SweepTemplateDto>,
    on_delete: Callback<SweepTemplateDto>,
    on_set_default: Callback<SweepTemplateDto>,
    on_duplicate: Callback<SweepTemplateDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let t_edit = template.clone();
    let t_delete = template.clone();
    let t_default = template.clone();
    let t_duplicate = template.clone();
    let name = template.name.clone();
    let is_default = template.is_default;
    let member_count = template.members.len();
    let member_names: Vec<String> = template
        .members
        .iter()
        .map(|id| {
            lookup
                .with_value(|m| m.get(id).cloned())
                .unwrap_or_else(|| format!("(missing {id})"))
        })
        .collect();

    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <div class="flex items-center gap-2">
                    <h3 class="section-title">{name}</h3>
                    {is_default.then(|| view! { <span class="pill pill-accent">"Default"</span> })}
                </div>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{format!("{member_count} configurations")}</span>
                    {member_names.into_iter().map(|n| view! {
                        <span class="pill pill-neutral">{n}</span>
                    }).collect_view()}
                </div>
            </div>
            <div class="flex gap-2 shrink-0 flex-wrap">
                <button
                    type="button"
                    class="btn"
                    disabled=Signal::derive(move || busy.get() || is_default)
                    on:click=move |_| on_set_default.run(t_default.clone())
                >
                    "Set default"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_duplicate.run(t_duplicate.clone())
                >
                    "Duplicate"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_edit.run(t_edit.clone())
                >
                    "Edit"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_delete.run(t_delete.clone())
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}

#[component]
fn SweepTemplateFormDialog(
    form_mode: ReadSignal<Option<SweepFormMode>>,
    set_form_mode: WriteSignal<Option<SweepFormMode>>,
    chunking_configs_resource: Resource<Result<Vec<ChunkingConfigurationDto>, String>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (selected, set_selected) = signal::<Vec<Uuid>>(Vec::new());
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form_mode.get() {
            None => {}
            Some(SweepFormMode::Add) => {
                set_name.set(String::new());
                set_selected.set(Vec::new());
            }
            Some(SweepFormMode::Edit(t)) => {
                set_name.set(t.name);
                set_selected.set(t.members);
            }
        }
    });

    let close = Callback::new(move |_| {
        set_form_mode.set(None);
        set_dialog_error.set(None);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name_val = name.get().trim().to_string();
        if name_val.is_empty() {
            set_dialog_error.set(Some("Name is required.".into()));
            return;
        }
        let members = selected.get();
        if members.is_empty() {
            set_dialog_error.set(Some("Pick at least one chunking configuration.".into()));
            return;
        }
        let command = match form_mode.get() {
            Some(SweepFormMode::Add) => {
                SweepTemplateCommandDto::CreateSweepTemplate(CreateSweepTemplateDto {
                    name: name_val,
                    members,
                })
            }
            Some(SweepFormMode::Edit(t)) => {
                SweepTemplateCommandDto::UpdateSweepTemplate(UpdateSweepTemplateDto {
                    sweep_template_id: t.sweep_template_id,
                    name: name_val,
                    members,
                })
            }
            None => return,
        };
        run_sweep_template_command(
            command,
            "Sweep template saved",
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
                Some(SweepFormMode::Edit(_)) => "Edit sweep template".to_string(),
                _ => "New sweep template".to_string(),
            }).get()
            subtitle="Group chunking configurations under a name and use them as the variants axis on the evaluation launcher.".to_string()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|msg| view! {
                    <div class="log-line-error text-sm">{msg}</div>
                })}

                <LabelledInput
                    label="Name".to_string()
                    hint="e.g. bert-only-sweep, section-tight".to_string()
                    value=name
                    set_value=set_name
                />

                <div class="space-y-1.5">
                    <span class="eyebrow">"Chunking configurations"</span>
                    <SweepMemberPicker
                        chunking_configs_resource=chunking_configs_resource
                        selected=selected
                        set_selected=set_selected
                    />
                </div>

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
fn SweepMemberPicker(
    chunking_configs_resource: Resource<Result<Vec<ChunkingConfigurationDto>, String>>,
    selected: ReadSignal<Vec<Uuid>>,
    set_selected: WriteSignal<Vec<Uuid>>,
) -> impl IntoView {
    view! {
        <Transition fallback=|| view! { <p class="muted text-sm">"Loading chunking configurations…"</p> }>
            {move || match chunking_configs_resource.get() {
                Some(Ok(list)) if list.is_empty() => view! {
                    <p class="text-sm muted">"No chunking configurations available. Create some above first."</p>
                }.into_any(),
                Some(Ok(list)) => view! {
                    <div class="flex flex-col gap-1.5 max-h-64 overflow-y-auto">
                        {list.into_iter().map(|cc| {
                            let id = cc.chunking_configuration_id;
                            let label = cc.name.clone();
                            let descriptor = cc.config.describe();
                            let is_checked = move || selected.get().contains(&id);
                            view! {
                                <label class="flex items-center gap-2 px-2 py-1 rounded hover:bg-[var(--color-surface-2)] cursor-pointer">
                                    <input
                                        type="checkbox"
                                        prop:checked=is_checked
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            set_selected.update(|list| {
                                                if checked {
                                                    if !list.contains(&id) {
                                                        list.push(id);
                                                    }
                                                } else {
                                                    list.retain(|m| *m != id);
                                                }
                                            });
                                        }
                                    />
                                    <span class="text-sm text-text">{label}</span>
                                    <span class="text-xs faint">{descriptor}</span>
                                </label>
                            }
                        }).collect_view()}
                    </div>
                }.into_any(),
                Some(Err(e)) => view! {
                    <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                }.into_any(),
                None => ().into_any(),
            }}
        </Transition>
    }
}

#[component]
fn SweepTemplateDeleteDialog(
    target: ReadSignal<Option<SweepTemplateDto>>,
    set_target: WriteSignal<Option<SweepTemplateDto>>,
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
        run_sweep_template_command(
            SweepTemplateCommandDto::DeleteSweepTemplate(DeleteSweepTemplateDto {
                sweep_template_id: t.sweep_template_id,
            }),
            "Sweep template deleted",
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
            title="Delete sweep template".to_string()
            subtitle="The underlying chunking configurations stay in place; only this named bundle is removed.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised p-3 rounded">
                    <span class="muted text-sm">"Template"</span>
                    <div class="text-text">{move || target.get().map(|t| t.name).unwrap_or_default()}</div>
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
