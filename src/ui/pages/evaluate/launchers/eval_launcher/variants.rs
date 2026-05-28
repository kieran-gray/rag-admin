use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::SweepTemplateDto;
use crate::shared::reference_data::ChunkStrategy;
use crate::shared::{
    BertChunkingConfig, ChunkingConfig, ChunkingVariant, DarnChunkingConfig, DarnGranularity,
    LlmChunkingConfig, SectionChunkingConfig,
};

use super::widgets::{FieldRow, ModeRadio, NumField, StrategyPicker, TextField};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VariantsMode {
    Single,
    SweepTemplate,
    StrategySweep,
    Custom,
}

#[allow(clippy::too_many_arguments)]
#[component]
pub(super) fn VariantsPicker(
    variants_mode: ReadSignal<VariantsMode>,
    set_variants_mode: WriteSignal<VariantsMode>,
    single_strategy: ReadSignal<ChunkStrategy>,
    set_single_strategy: WriteSignal<ChunkStrategy>,
    section_tokens: ReadSignal<u32>,
    set_section_tokens: WriteSignal<u32>,
    bert_target: ReadSignal<u32>,
    set_bert_target: WriteSignal<u32>,
    bert_overlap: ReadSignal<u32>,
    set_bert_overlap: WriteSignal<u32>,
    llm_micro: ReadSignal<u32>,
    set_llm_micro: WriteSignal<u32>,
    darn_size: ReadSignal<u32>,
    set_darn_size: WriteSignal<u32>,
    darn_overlap: ReadSignal<u32>,
    set_darn_overlap: WriteSignal<u32>,
    sweep_strategy: ReadSignal<ChunkStrategy>,
    set_sweep_strategy: WriteSignal<ChunkStrategy>,
    sweep_section_tokens_input: ReadSignal<String>,
    set_sweep_section_tokens_input: WriteSignal<String>,
    sweep_bert_targets_input: ReadSignal<String>,
    set_sweep_bert_targets_input: WriteSignal<String>,
    sweep_bert_overlaps_input: ReadSignal<String>,
    set_sweep_bert_overlaps_input: WriteSignal<String>,
    sweep_llm_micro_input: ReadSignal<String>,
    set_sweep_llm_micro_input: WriteSignal<String>,
    sweep_darn_sizes_input: ReadSignal<String>,
    set_sweep_darn_sizes_input: WriteSignal<String>,
    custom_variants: ReadSignal<Vec<ChunkingVariant>>,
    set_custom_variants: WriteSignal<Vec<ChunkingVariant>>,
    sweep_templates: StoredValue<Vec<SweepTemplateDto>>,
    selected_sweep_template: ReadSignal<Option<Uuid>>,
    set_selected_sweep_template: WriteSignal<Option<Uuid>>,
    variants_computed: Memo<Result<Vec<ChunkingVariant>, String>>,
    active_gen_model: Signal<Uuid>,
) -> impl IntoView {
    view! {
        <div class="space-y-3 pt-3">
            <ModeRadio<VariantsMode>
                value=variants_mode
                set_value=set_variants_mode
                options=vec![
                    (VariantsMode::Single, "Single"),
                    (VariantsMode::StrategySweep, "Sweep one strategy"),
                    (VariantsMode::SweepTemplate, "Sweep template"),
                    (VariantsMode::Custom, "Custom list"),
                ]
            />

            {move || match variants_mode.get() {
                VariantsMode::Single => view! {
                    <SingleVariantFields
                        strategy=single_strategy
                        set_strategy=set_single_strategy
                        section_tokens=section_tokens
                        set_section_tokens=set_section_tokens
                        bert_target=bert_target
                        set_bert_target=set_bert_target
                        bert_overlap=bert_overlap
                        set_bert_overlap=set_bert_overlap
                        llm_micro=llm_micro
                        set_llm_micro=set_llm_micro
                        darn_size=darn_size
                        set_darn_size=set_darn_size
                        darn_overlap=darn_overlap
                        set_darn_overlap=set_darn_overlap
                    />
                }.into_any(),
                VariantsMode::StrategySweep => view! {
                    <StrategySweepFields
                        strategy=sweep_strategy
                        set_strategy=set_sweep_strategy
                        section_input=sweep_section_tokens_input
                        set_section_input=set_sweep_section_tokens_input
                        bert_targets_input=sweep_bert_targets_input
                        set_bert_targets_input=set_sweep_bert_targets_input
                        bert_overlaps_input=sweep_bert_overlaps_input
                        set_bert_overlaps_input=set_sweep_bert_overlaps_input
                        llm_micro_input=sweep_llm_micro_input
                        set_llm_micro_input=set_sweep_llm_micro_input
                        darn_sizes_input=sweep_darn_sizes_input
                        set_darn_sizes_input=set_sweep_darn_sizes_input
                    />
                }.into_any(),
                VariantsMode::SweepTemplate => view! {
                    <SweepTemplatePicker
                        sweep_templates=sweep_templates
                        selected=selected_sweep_template
                        set_selected=set_selected_sweep_template
                    />
                }.into_any(),
                VariantsMode::Custom => view! {
                    <CustomVariantsList
                        custom_variants=custom_variants
                        set_custom_variants=set_custom_variants
                        active_gen_model=active_gen_model
                    />
                }.into_any(),
            }}

            {move || match variants_computed.get() {
                Ok(list) => view! {
                    <div class="flex flex-wrap gap-1.5">
                        {list.into_iter().map(|v| view! {
                            <span class="pill pill-neutral">{v.label}</span>
                        }).collect_view()}
                    </div>
                }.into_any(),
                Err(e) => view! {
                    <div class="text-sm log-line-error">{e}</div>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn SingleVariantFields(
    strategy: ReadSignal<ChunkStrategy>,
    set_strategy: WriteSignal<ChunkStrategy>,
    section_tokens: ReadSignal<u32>,
    set_section_tokens: WriteSignal<u32>,
    bert_target: ReadSignal<u32>,
    set_bert_target: WriteSignal<u32>,
    bert_overlap: ReadSignal<u32>,
    set_bert_overlap: WriteSignal<u32>,
    llm_micro: ReadSignal<u32>,
    set_llm_micro: WriteSignal<u32>,
    darn_size: ReadSignal<u32>,
    set_darn_size: WriteSignal<u32>,
    darn_overlap: ReadSignal<u32>,
    set_darn_overlap: WriteSignal<u32>,
) -> impl IntoView {
    view! {
        <div class="space-y-3">
            <StrategyPicker value=strategy set_value=set_strategy />
            {move || match strategy.get() {
                ChunkStrategy::Section => view! {
                    <FieldRow>
                        <NumField label="Max section tokens".to_string() value=section_tokens set_value=set_section_tokens min=1 />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Bert => view! {
                    <FieldRow>
                        <NumField label="Target tokens".to_string() value=bert_target set_value=set_bert_target min=1 />
                        <NumField label="Overlap tokens".to_string() value=bert_overlap set_value=set_bert_overlap min=0 />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Llm => view! {
                    <FieldRow>
                        <NumField label="Micro-chunk tokens".to_string() value=llm_micro set_value=set_llm_micro min=32 />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Darn => view! {
                    <FieldRow>
                        <NumField label="Max chunk size".to_string() value=darn_size set_value=set_darn_size min=1 />
                        <NumField label="Overlap".to_string() value=darn_overlap set_value=set_darn_overlap min=0 />
                    </FieldRow>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn StrategySweepFields(
    strategy: ReadSignal<ChunkStrategy>,
    set_strategy: WriteSignal<ChunkStrategy>,
    section_input: ReadSignal<String>,
    set_section_input: WriteSignal<String>,
    bert_targets_input: ReadSignal<String>,
    set_bert_targets_input: WriteSignal<String>,
    bert_overlaps_input: ReadSignal<String>,
    set_bert_overlaps_input: WriteSignal<String>,
    llm_micro_input: ReadSignal<String>,
    set_llm_micro_input: WriteSignal<String>,
    darn_sizes_input: ReadSignal<String>,
    set_darn_sizes_input: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <div class="space-y-3">
            <StrategyPicker value=strategy set_value=set_strategy />
            {move || match strategy.get() {
                ChunkStrategy::Section => view! {
                    <FieldRow>
                        <TextField
                            label="Section token values".to_string()
                            hint="e.g. 256,384,480,512 or 256-512:64"
                            value=section_input
                            set_value=set_section_input
                        />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Bert => view! {
                    <FieldRow>
                        <TextField
                            label="Target values".to_string()
                            hint="e.g. 256,320,384,448"
                            value=bert_targets_input
                            set_value=set_bert_targets_input
                        />
                        <TextField
                            label="Overlap values".to_string()
                            hint="e.g. 0,48,64"
                            value=bert_overlaps_input
                            set_value=set_bert_overlaps_input
                        />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Llm => view! {
                    <FieldRow>
                        <TextField
                            label="Micro-chunk token values".to_string()
                            hint="e.g. 64,96,128"
                            value=llm_micro_input
                            set_value=set_llm_micro_input
                        />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Darn => view! {
                    <FieldRow>
                        <TextField
                            label="Darn chunk sizes".to_string()
                            hint="e.g. 300,500,800,1000"
                            value=darn_sizes_input
                            set_value=set_darn_sizes_input
                        />
                    </FieldRow>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn CustomVariantsList(
    custom_variants: ReadSignal<Vec<ChunkingVariant>>,
    set_custom_variants: WriteSignal<Vec<ChunkingVariant>>,
    active_gen_model: Signal<Uuid>,
) -> impl IntoView {
    let (draft_strategy, set_draft_strategy) = signal(ChunkStrategy::Section);
    let (draft_tokens, set_draft_tokens) = signal(512u32);
    let (draft_overlap, set_draft_overlap) = signal(64u32);
    let (draft_micro, set_draft_micro) = signal(96u32);
    let (draft_darn_size, set_draft_darn_size) = signal(500u32);
    let (draft_darn_overlap, set_draft_darn_overlap) = signal(50u32);

    let add = move |_| {
        let v = match draft_strategy.get() {
            ChunkStrategy::Section => ChunkingVariant {
                label: format!("section:{}", draft_tokens.get()),
                config: ChunkingConfig::Section(SectionChunkingConfig {
                    max_section_tokens: draft_tokens.get(),
                }),
            },
            ChunkStrategy::Bert => ChunkingVariant {
                label: format!("bert:{}/{}", draft_tokens.get(), draft_overlap.get()),
                config: ChunkingConfig::Bert(BertChunkingConfig {
                    target_tokens: draft_tokens.get(),
                    overlap_tokens: draft_overlap.get(),
                    min_tokens: 96,
                }),
            },
            ChunkStrategy::Llm => ChunkingVariant {
                label: format!("llm:{}", draft_micro.get()),
                config: ChunkingConfig::Llm(LlmChunkingConfig {
                    target_tokens: 384,
                    micro_chunk_tokens: draft_micro.get(),
                    generation_model_id: active_gen_model.get(),
                }),
            },
            ChunkStrategy::Darn => ChunkingVariant {
                label: format!(
                    "darn:{}/{}",
                    draft_darn_size.get(),
                    draft_darn_overlap.get()
                ),
                config: ChunkingConfig::Darn(DarnChunkingConfig {
                    max_chunk_size: draft_darn_size.get(),
                    overlap: draft_darn_overlap.get(),
                    granularity: DarnGranularity::Characters,
                }),
            },
        };
        set_custom_variants.update(|list| {
            if !list.iter().any(|existing| existing.label == v.label) {
                list.push(v);
            }
        });
    };

    view! {
        <div class="space-y-3">
            <StrategyPicker value=draft_strategy set_value=set_draft_strategy />
            <div class="flex items-end gap-2 flex-wrap">
                {move || match draft_strategy.get() {
                    ChunkStrategy::Section => view! {
                        <NumField label="Max section tokens".to_string() value=draft_tokens set_value=set_draft_tokens min=1 />
                    }.into_any(),
                    ChunkStrategy::Bert => view! {
                        <div class="flex gap-2">
                            <NumField label="Target tokens".to_string() value=draft_tokens set_value=set_draft_tokens min=1 />
                            <NumField label="Overlap tokens".to_string() value=draft_overlap set_value=set_draft_overlap min=0 />
                        </div>
                    }.into_any(),
                    ChunkStrategy::Llm => view! {
                        <NumField label="Micro-chunk tokens".to_string() value=draft_micro set_value=set_draft_micro min=32 />
                    }.into_any(),
                    ChunkStrategy::Darn => view! {
                        <div class="flex gap-2">
                            <NumField label="Max chunk size".to_string() value=draft_darn_size set_value=set_draft_darn_size min=1 />
                            <NumField label="Overlap".to_string() value=draft_darn_overlap set_value=set_draft_darn_overlap min=0 />
                        </div>
                    }.into_any(),
                }}
                <button type="button" class="btn" on:click=add>"+ Add variant"</button>
            </div>
            <div class="flex flex-wrap gap-1.5">
                {move || custom_variants.get().into_iter().enumerate().map(|(i, v)| {
                    let label = v.label.clone();
                    view! {
                        <span class="pill pill-neutral inline-flex items-center gap-1.5">
                            {label}
                            <button
                                type="button"
                                class="faint hover:text-text"
                                aria-label="Remove variant"
                                on:click=move |_| set_custom_variants.update(|list| { list.remove(i); })
                            >
                                "✕"
                            </button>
                        </span>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn SweepTemplatePicker(
    sweep_templates: StoredValue<Vec<SweepTemplateDto>>,
    selected: ReadSignal<Option<Uuid>>,
    set_selected: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    let templates = sweep_templates.with_value(Clone::clone);
    if templates.is_empty() {
        return view! {
            <p class="text-sm muted">
                "No sweep templates defined. Falling back to every chunking configuration in the registry. Create a template on /pipeline/chunking to scope the sweep."
            </p>
        }
        .into_any();
    }

    view! {
        <div class="flex items-center gap-2 flex-wrap">
            <span class="eyebrow shrink-0">"Template"</span>
            <select
                class="input max-w-md"
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    if let Ok(id) = v.parse::<Uuid>() {
                        set_selected.set(Some(id));
                    }
                }
            >
                {templates.into_iter().map(|t| {
                    let id = t.sweep_template_id;
                    let is_selected = move || selected.get() == Some(id);
                    let label = if t.is_default {
                        format!("{} (default) · {} configs", t.name, t.members.len())
                    } else {
                        format!("{} · {} configs", t.name, t.members.len())
                    };
                    view! {
                        <option value=id.to_string() selected=is_selected>
                            {label}
                        </option>
                    }
                }).collect_view()}
            </select>
        </div>
    }
    .into_any()
}
