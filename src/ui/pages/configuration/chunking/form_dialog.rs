use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{
    ChunkingConfigurationCommandDto, ConfigurationDto, CreateChunkingConfigurationDto,
    UpdateChunkingConfigurationDto,
};
use crate::shared::reference_data::ChunkStrategy;
use crate::shared::{
    BertChunkingConfig, ChunkingConfig, DarnChunkingConfig, DarnGranularity, LlmChunkingConfig,
    SectionChunkingConfig,
};
use crate::ui::components::primitives::Dialog;
use crate::ui::pages::configuration::commands::{
    parse_uuid_or_none, run_chunking_configuration_command,
};

use super::widgets::{LabelledInput, NumberField};
use super::FormMode;

#[component]
pub(super) fn ChunkingFormDialog(
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
    let (darn_cfg, set_darn_cfg) = signal(DarnChunkingConfig::default());
    let (is_default, set_is_default) = signal(false);
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
                set_darn_cfg.set(DarnChunkingConfig::default());
                set_is_default.set(false);
            }
            Some(FormMode::Edit(cc)) => {
                set_name.set(cc.name);
                set_strategy.set(cc.config.strategy());
                set_is_default.set(cc.is_default);
                match cc.config {
                    ChunkingConfig::Section(c) => set_section_cfg.set(c),
                    ChunkingConfig::Bert(c) => set_bert_cfg.set(c),
                    ChunkingConfig::Llm(c) => set_llm_cfg.set(c),
                    ChunkingConfig::Darn(c) => set_darn_cfg.set(c),
                }
            }
        }
    });

    let current_config = move || match strategy.get() {
        ChunkStrategy::Section => ChunkingConfig::Section(section_cfg.get()),
        ChunkStrategy::Bert => ChunkingConfig::Bert(bert_cfg.get()),
        ChunkStrategy::Llm => ChunkingConfig::Llm(llm_cfg.get()),
        ChunkStrategy::Darn => ChunkingConfig::Darn(darn_cfg.get()),
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
        let default_flag = is_default.get();
        let command = match form_mode.get() {
            Some(FormMode::Add) => ChunkingConfigurationCommandDto::CreateChunkingConfiguration(
                CreateChunkingConfigurationDto {
                    name: name_val,
                    config,
                    is_default: default_flag,
                },
            ),
            Some(FormMode::Edit(cc)) => {
                ChunkingConfigurationCommandDto::UpdateChunkingConfiguration(
                    UpdateChunkingConfigurationDto {
                        chunking_configuration_id: cc.chunking_configuration_id,
                        name: name_val,
                        config,
                        is_default: default_flag,
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
                    darn_cfg=darn_cfg
                    set_darn_cfg=set_darn_cfg
                    generation_models=Memo::new(move |_| config.with_value(|c| {
                        c.generation_models
                            .iter()
                            .map(|m| (m.generation_model_id, m.model.clone()))
                            .collect::<Vec<_>>()
                    }))
                />

                <label class="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        prop:checked=is_default
                        on:change=move |ev| set_is_default.set(event_target_checked(&ev))
                    />
                    <span>"Use as default chunking configuration"</span>
                </label>

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
    darn_cfg: ReadSignal<DarnChunkingConfig>,
    set_darn_cfg: WriteSignal<DarnChunkingConfig>,
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
            ChunkStrategy::Darn => view! {
                <div class="space-y-3">
                    <NumberField
                        label="MAX_CHUNK_SIZE".to_string()
                        hint="darn: maximum chunk size (characters or tokens)".to_string()
                        min=1
                        value=Signal::derive(move || darn_cfg.get().max_chunk_size)
                        on_change=Callback::new(move |v: u32| {
                            set_darn_cfg.update(|c| c.max_chunk_size = v);
                        })
                    />
                    <NumberField
                        label="OVERLAP".to_string()
                        hint="darn: characters or tokens repeated at chunk boundaries".to_string()
                        min=0
                        value=Signal::derive(move || darn_cfg.get().overlap)
                        on_change=Callback::new(move |v: u32| {
                            set_darn_cfg.update(|c| c.overlap = v);
                        })
                    />
                    <label class="block space-y-1.5">
                        <span class="eyebrow">"GRANULARITY"</span>
                        <div class="flex gap-2 flex-wrap">
                            <button
                                type="button"
                                class=move || {
                                    if darn_cfg.get().granularity == DarnGranularity::Characters {
                                        "btn btn-primary"
                                    } else {
                                        "btn"
                                    }
                                }
                                on:click=move |_| set_darn_cfg.update(|c| c.granularity = DarnGranularity::Characters)
                            >
                                "characters"
                            </button>
                            <button
                                type="button"
                                class=move || {
                                    if darn_cfg.get().granularity == DarnGranularity::Tokens {
                                        "btn btn-primary"
                                    } else {
                                        "btn"
                                    }
                                }
                                on:click=move |_| set_darn_cfg.update(|c| c.granularity = DarnGranularity::Tokens)
                            >
                                "tokens"
                            </button>
                        </div>
                        <span class="text-xs faint">"Locked at config time; not swept by evaluation."</span>
                    </label>
                </div>
            }.into_any(),
        }}
    }
}
