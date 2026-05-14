use leptos::prelude::*;
use uuid::Uuid;

use crate::components::primitives::Surface;
use crate::shared::{
    BertChunkingConfig, ChunkStrategy, ChunkingConfig, ChunkingConfigurationDto, ChunkingVariant,
    EvaluationAutotuneRequest, EvaluationRunOptions, LlmChunkingConfig, PipelineConfigurationDto,
    RunEvaluationRequestDto, SectionChunkingConfig, SweepTemplateDto,
};

use super::eval_parser::parse_u32_values;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariantsMode {
    Single,
    SweepTemplate,
    StrategySweep,
    Custom,
}

#[allow(dead_code)]
const SWEEP_TEMPLATE_STORAGE_KEY: &str = "eval_launcher.sweep_template_id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionsMode {
    Single,
    Sweep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    ScoreAll,
    Autotune,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preset {
    FindBestChunking,
    TuneRetrieval,
    FullSweep,
    TestOne,
}

impl Preset {
    fn label(self) -> &'static str {
        match self {
            Self::FindBestChunking => "Find best chunking",
            Self::TuneRetrieval => "Tune retrieval",
            Self::FullSweep => "Full sweep",
            Self::TestOne => "Test one",
        }
    }
}

#[derive(Clone)]
pub struct LauncherCallbacks {
    pub on_start: Callback<RunEvaluationRequestDto>,
}

#[component]
pub fn EvaluationLauncher(
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
    sweep_templates: StoredValue<Vec<SweepTemplateDto>>,
    active_dataset: ReadSignal<Option<Uuid>>,
    active_pipeline: ReadSignal<Option<Uuid>>,
    set_active_pipeline: WriteSignal<Option<Uuid>>,
    running: ReadSignal<bool>,
    callbacks: LauncherCallbacks,
) -> impl IntoView {
    let (variants_mode, set_variants_mode) = signal(VariantsMode::SweepTemplate);
    let (variants_expanded, set_variants_expanded) = signal(false);
    let (single_strategy, set_single_strategy) = signal(ChunkStrategy::Section);
    let (section_tokens, set_section_tokens) = signal(512u32);
    let (bert_target, set_bert_target) = signal(384u32);
    let (bert_overlap, set_bert_overlap) = signal(64u32);
    let (llm_micro, set_llm_micro) = signal(96u32);

    let (sweep_strategy, set_sweep_strategy) = signal(ChunkStrategy::Section);
    let (sweep_section_tokens_input, set_sweep_section_tokens_input) =
        signal("256,384,480,512".to_string());
    let (sweep_bert_targets_input, set_sweep_bert_targets_input) =
        signal("256,320,384,448".to_string());
    let (sweep_bert_overlaps_input, set_sweep_bert_overlaps_input) = signal("0,48,64".to_string());
    let (sweep_llm_micro_input, set_sweep_llm_micro_input) = signal("64,96,128".to_string());

    let (custom_variants, set_custom_variants) = signal::<Vec<ChunkingVariant>>(Vec::new());

    let initial_sweep_template_id = sweep_templates.with_value(|tpls| {
        let stored = load_sweep_template_pref()
            .and_then(|id| tpls.iter().find(|t| t.sweep_template_id == id))
            .map(|t| t.sweep_template_id);
        stored
            .or_else(|| {
                tpls.iter()
                    .find(|t| t.is_default)
                    .map(|t| t.sweep_template_id)
            })
            .or_else(|| tpls.first().map(|t| t.sweep_template_id))
    });
    let (selected_sweep_template, set_selected_sweep_template) =
        signal::<Option<Uuid>>(initial_sweep_template_id);
    Effect::new(move |_| {
        let id = selected_sweep_template.get();
        store_sweep_template_pref(id);
    });

    let (options_mode, set_options_mode) = signal(OptionsMode::Single);
    let (options_expanded, set_options_expanded) = signal(false);
    let (single_top_k, set_single_top_k) = signal(5u32);
    let (single_min_score_milli, set_single_min_score_milli) = signal(0u32);
    let (sweep_top_k_input, set_sweep_top_k_input) = signal("2,3,5,8".to_string());
    let (sweep_min_score_input, set_sweep_min_score_input) = signal("0,200,500,800".to_string());

    let (run_mode, set_run_mode) = signal(RunMode::ScoreAll);

    let apply_preset = move |p: Preset| match p {
        Preset::FindBestChunking => {
            set_variants_mode.set(VariantsMode::SweepTemplate);
            set_options_mode.set(OptionsMode::Single);
            set_run_mode.set(RunMode::ScoreAll);
        }
        Preset::TuneRetrieval => {
            set_variants_mode.set(VariantsMode::Single);
            set_options_mode.set(OptionsMode::Sweep);
            set_run_mode.set(RunMode::ScoreAll);
        }
        Preset::FullSweep => {
            set_variants_mode.set(VariantsMode::SweepTemplate);
            set_options_mode.set(OptionsMode::Sweep);
            set_run_mode.set(RunMode::ScoreAll);
        }
        Preset::TestOne => {
            set_variants_mode.set(VariantsMode::Single);
            set_options_mode.set(OptionsMode::Single);
            set_run_mode.set(RunMode::ScoreAll);
        }
    };

    let variants_computed = Memo::new(move |_| match variants_mode.get() {
        VariantsMode::Single => build_single_variant(
            single_strategy.get(),
            section_tokens.get(),
            bert_target.get(),
            bert_overlap.get(),
            llm_micro.get(),
        )
        .map(|v| vec![v])
        .map_err(|e| format!("variant error: {e}")),
        VariantsMode::SweepTemplate => {
            let templates = sweep_templates.with_value(|t| t.clone());
            let configs = chunking_configurations.with_value(|c| c.clone());
            if templates.is_empty() {
                let seeded = default_sweep_variants(&configs);
                if seeded.is_empty() {
                    Err("No sweep templates or chunking configurations in the registry. Create some on /chunking.".into())
                } else {
                    Ok(seeded)
                }
            } else {
                let id = selected_sweep_template.get();
                let template = id
                    .and_then(|id| templates.iter().find(|t| t.sweep_template_id == id))
                    .or_else(|| templates.iter().find(|t| t.is_default))
                    .or_else(|| templates.first());
                let Some(template) = template else {
                    return Err("Pick a sweep template.".into());
                };
                let variants = template_variants(template, &configs);
                if variants.is_empty() {
                    Err(format!(
                        "Sweep template '{}' has no resolvable chunking configurations. Edit it on /chunking.",
                        template.name
                    ))
                } else {
                    Ok(variants)
                }
            }
        }
        VariantsMode::StrategySweep => match sweep_strategy.get() {
            ChunkStrategy::Section => {
                parse_u32_values(&sweep_section_tokens_input.get(), 1, 4096, 64)
                    .map(build_section_sweep)
                    .map_err(|e| format!("section sweep: {e}"))
            }
            ChunkStrategy::Bert => {
                let targets = parse_u32_values(&sweep_bert_targets_input.get(), 1, 4096, 64);
                let overlaps = parse_u32_values(&sweep_bert_overlaps_input.get(), 0, 1024, 16);
                match (targets, overlaps) {
                    (Ok(t), Ok(o)) => Ok(build_bert_sweep(&t, &o)),
                    (Err(e), _) => Err(format!("bert target sweep: {e}")),
                    (_, Err(e)) => Err(format!("bert overlap sweep: {e}")),
                }
            }
            ChunkStrategy::Llm => parse_u32_values(&sweep_llm_micro_input.get(), 32, 1024, 32)
                .map(build_llm_sweep)
                .map_err(|e| format!("llm sweep: {e}")),
        },
        VariantsMode::Custom => {
            let list = custom_variants.get();
            if list.is_empty() {
                Err("Add at least one variant".into())
            } else {
                Ok(list)
            }
        }
    });

    let options_computed: Memo<Result<Vec<EvaluationRunOptions>, String>> =
        Memo::new(move |_| match options_mode.get() {
            OptionsMode::Single => Ok(vec![EvaluationRunOptions {
                top_k: single_top_k.get(),
                min_score_milli: single_min_score_milli.get(),
            }]),
            OptionsMode::Sweep => {
                let top_ks = parse_u32_values(&sweep_top_k_input.get(), 1, 100, 1)
                    .map_err(|e| format!("top-k: {e}"))?;
                let min_scores = parse_u32_values(&sweep_min_score_input.get(), 0, 1000, 100)
                    .map_err(|e| format!("min-score: {e}"))?;

                let mut combos = Vec::with_capacity(top_ks.len() * min_scores.len());
                for &t in &top_ks {
                    for &m in &min_scores {
                        combos.push(EvaluationRunOptions {
                            top_k: t,
                            min_score_milli: m,
                        });
                    }
                }
                Ok(combos)
            }
        });

    let cost_summary = Memo::new(move |_| {
        let vc: Result<usize, String> = variants_computed.with(|r| match r {
            Ok(v) => Ok(v.len()),
            Err(e) => Err(e.clone()),
        });
        let oc: Result<usize, String> = options_computed.with(|r| match r {
            Ok(o) => Ok(o.len()),
            Err(e) => Err(e.clone()),
        });
        match (vc, oc) {
            (Ok(v), Ok(o)) => Ok((v, o, v * o)),
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    });

    let on_submit = move || {
        let Some(dataset_id) = active_dataset.get() else {
            return;
        };
        let Some(pipeline_id) = active_pipeline.get() else {
            return;
        };
        let variants = match variants_computed.get() {
            Ok(v) if !v.is_empty() => v,
            _ => return,
        };
        let options = match options_computed.get() {
            Ok(o) if !o.is_empty() => o,
            _ => return,
        };

        let autotune = match run_mode.get() {
            RunMode::ScoreAll => None,
            RunMode::Autotune => Some(EvaluationAutotuneRequest {
                current_config: variants.first().map(|v| v.config).unwrap_or_default(),
                top_k_values: options.iter().map(|o| o.top_k).collect::<Vec<_>>(),
                min_score_milli_values: options
                    .iter()
                    .map(|o| o.min_score_milli)
                    .collect::<Vec<_>>(),
            }),
        };

        callbacks.on_start.run(RunEvaluationRequestDto {
            dataset_id,
            pipeline_configuration_id: pipeline_id,
            variants,
            options,
            autotune,
        });
    };

    let can_start = move || {
        !running.get()
            && active_dataset.get().is_some()
            && active_pipeline.get().is_some()
            && cost_summary.with(|c| c.is_ok())
    };

    view! {
        <Surface title="Tune for best chunking".to_string()>
            <div class="space-y-5">
                <div class="flex items-center gap-2 flex-wrap">
                    <span class="eyebrow shrink-0">"Presets"</span>
                    {[Preset::FindBestChunking, Preset::TuneRetrieval, Preset::FullSweep, Preset::TestOne]
                        .into_iter().map(|p| view! {
                            <button
                                type="button"
                                class="btn btn-ghost"
                                on:click=move |_| apply_preset(p)
                            >
                                {p.label()}
                            </button>
                        }).collect_view()}
                </div>

                <PipelineRow
                    pipelines=pipelines
                    active_pipeline=active_pipeline
                    set_active_pipeline=set_active_pipeline
                />

                <Section
                    title="Chunking variants".to_string()
                    summary=Signal::derive(move || variants_summary(variants_computed))
                    expanded=variants_expanded
                    set_expanded=set_variants_expanded
                >
                    <VariantsPicker
                        variants_mode=variants_mode
                        set_variants_mode=set_variants_mode
                        single_strategy=single_strategy
                        set_single_strategy=set_single_strategy
                        section_tokens=section_tokens
                        set_section_tokens=set_section_tokens
                        bert_target=bert_target
                        set_bert_target=set_bert_target
                        bert_overlap=bert_overlap
                        set_bert_overlap=set_bert_overlap
                        llm_micro=llm_micro
                        set_llm_micro=set_llm_micro
                        sweep_strategy=sweep_strategy
                        set_sweep_strategy=set_sweep_strategy
                        sweep_section_tokens_input=sweep_section_tokens_input
                        set_sweep_section_tokens_input=set_sweep_section_tokens_input
                        sweep_bert_targets_input=sweep_bert_targets_input
                        set_sweep_bert_targets_input=set_sweep_bert_targets_input
                        sweep_bert_overlaps_input=sweep_bert_overlaps_input
                        set_sweep_bert_overlaps_input=set_sweep_bert_overlaps_input
                        sweep_llm_micro_input=sweep_llm_micro_input
                        set_sweep_llm_micro_input=set_sweep_llm_micro_input
                        custom_variants=custom_variants
                        set_custom_variants=set_custom_variants
                        sweep_templates=sweep_templates
                        selected_sweep_template=selected_sweep_template
                        set_selected_sweep_template=set_selected_sweep_template
                        variants_computed=variants_computed
                    />
                </Section>

                <Section
                    title="Retrieval options".to_string()
                    summary=Signal::derive(move || options_summary(options_computed))
                    expanded=options_expanded
                    set_expanded=set_options_expanded
                >
                    <OptionsPicker
                        options_mode=options_mode
                        set_options_mode=set_options_mode
                        single_top_k=single_top_k
                        set_single_top_k=set_single_top_k
                        single_min_score_milli=single_min_score_milli
                        set_single_min_score_milli=set_single_min_score_milli
                        sweep_top_k_input=sweep_top_k_input
                        set_sweep_top_k_input=set_sweep_top_k_input
                        sweep_min_score_input=sweep_min_score_input
                        set_sweep_min_score_input=set_sweep_min_score_input
                        options_computed=options_computed
                    />
                </Section>

                <RunModePicker run_mode=run_mode set_run_mode=set_run_mode />

                <div class="flex items-center justify-between gap-4 pt-3 border-t border-[var(--color-border)]">
                    <CostSummary cost=cost_summary />
                    <button
                        type="button"
                        class="btn btn-primary"
                        disabled=move || !can_start()
                        on:click=move |_| on_submit()
                    >
                        {move || if running.get() {
                            "Running…"
                        } else if active_dataset.get().is_none() {
                            "Select a dataset"
                        } else if active_pipeline.get().is_none() {
                            "Select a pipeline"
                        } else if cost_summary.with(|c| c.is_err()) {
                            "Fix errors above"
                        } else {
                            "Start tuning"
                        }}
                    </button>
                </div>
            </div>
        </Surface>
    }
}

#[component]
fn Section(
    title: String,
    summary: Signal<String>,
    expanded: ReadSignal<bool>,
    set_expanded: WriteSignal<bool>,
    children: ChildrenFn,
) -> impl IntoView {
    let children = StoredValue::new(children);
    view! {
        <div class="rounded border border-[var(--color-border)]">
            <button
                type="button"
                class="w-full flex items-center justify-between gap-3 px-3 py-2.5 text-left hover:bg-[var(--color-surface-2)] transition-colors"
                on:click=move |_| set_expanded.update(|e| *e = !*e)
            >
                <div class="flex items-center gap-3 min-w-0">
                    <span class="text-text font-medium shrink-0">{title}</span>
                    <span class="text-sm muted truncate">{move || summary.get()}</span>
                </div>
                <span class="faint shrink-0">{move || if expanded.get() { "▴" } else { "▾" }}</span>
            </button>
            {move || expanded.get().then(|| view! {
                <div class="px-3 pb-3 border-t border-[var(--color-border)]">
                    {children.with_value(|c| c())}
                </div>
            })}
        </div>
    }
}

#[component]
fn PipelineRow(
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
    active_pipeline: ReadSignal<Option<Uuid>>,
    set_active_pipeline: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-3">
            <span class="eyebrow shrink-0">"Pipeline"</span>
            <select
                class="input max-w-md"
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    if v.is_empty() {
                        set_active_pipeline.set(None);
                    } else if let Ok(id) = v.parse::<Uuid>() {
                        set_active_pipeline.set(Some(id));
                    }
                }
            >
                <option value="">"— select pipeline —"</option>
                {move || {
                    let ps = pipelines.get_value();
                    ps.into_iter().map(|pc| {
                        let id = pc.pipeline_configuration_id;
                        let selected = active_pipeline.get() == Some(id);
                        view! {
                            <option value=id.to_string() selected=selected>
                                {pc.name}
                            </option>
                        }
                    }).collect_view()
                }}
            </select>
        </div>
    }
}

#[component]
fn VariantsPicker(
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
    custom_variants: ReadSignal<Vec<ChunkingVariant>>,
    set_custom_variants: WriteSignal<Vec<ChunkingVariant>>,
    sweep_templates: StoredValue<Vec<SweepTemplateDto>>,
    selected_sweep_template: ReadSignal<Option<Uuid>>,
    set_selected_sweep_template: WriteSignal<Option<Uuid>>,
    variants_computed: Memo<Result<Vec<ChunkingVariant>, String>>,
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
) -> impl IntoView {
    view! {
        <div class="space-y-3">
            <StrategyPicker value=strategy set_value=set_strategy />
            {move || match strategy.get() {
                ChunkStrategy::Section => view! {
                    <FieldRow>
                        <TextField
                            label="Section token values".to_string()
                            hint="e.g. 256,384,480,512 or 256-512:64".to_string()
                            value=section_input
                            set_value=set_section_input
                        />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Bert => view! {
                    <FieldRow>
                        <TextField
                            label="Target values".to_string()
                            hint="e.g. 256,320,384,448".to_string()
                            value=bert_targets_input
                            set_value=set_bert_targets_input
                        />
                        <TextField
                            label="Overlap values".to_string()
                            hint="e.g. 0,48,64".to_string()
                            value=bert_overlaps_input
                            set_value=set_bert_overlaps_input
                        />
                    </FieldRow>
                }.into_any(),
                ChunkStrategy::Llm => view! {
                    <FieldRow>
                        <TextField
                            label="Micro-chunk token values".to_string()
                            hint="e.g. 64,96,128".to_string()
                            value=llm_micro_input
                            set_value=set_llm_micro_input
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
) -> impl IntoView {
    let (draft_strategy, set_draft_strategy) = signal(ChunkStrategy::Section);
    let (draft_tokens, set_draft_tokens) = signal(512u32);
    let (draft_overlap, set_draft_overlap) = signal(64u32);
    let (draft_micro, set_draft_micro) = signal(96u32);

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
                    generation_model_id: Uuid::nil(),
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
fn OptionsPicker(
    options_mode: ReadSignal<OptionsMode>,
    set_options_mode: WriteSignal<OptionsMode>,
    single_top_k: ReadSignal<u32>,
    set_single_top_k: WriteSignal<u32>,
    single_min_score_milli: ReadSignal<u32>,
    set_single_min_score_milli: WriteSignal<u32>,
    sweep_top_k_input: ReadSignal<String>,
    set_sweep_top_k_input: WriteSignal<String>,
    sweep_min_score_input: ReadSignal<String>,
    set_sweep_min_score_input: WriteSignal<String>,
    options_computed: Memo<Result<Vec<EvaluationRunOptions>, String>>,
) -> impl IntoView {
    view! {
        <div class="space-y-3 pt-3">
            <ModeRadio<OptionsMode>
                value=options_mode
                set_value=set_options_mode
                options=vec![
                    (OptionsMode::Single, "Single set"),
                    (OptionsMode::Sweep, "Sweep across values"),
                ]
            />

            {move || match options_mode.get() {
                OptionsMode::Single => view! {
                    <FieldRow>
                        <NumField label="top-k".to_string() value=single_top_k set_value=set_single_top_k min=1 />
                        <NumField label="min-score (milli)".to_string() value=single_min_score_milli set_value=set_single_min_score_milli min=0 />
                    </FieldRow>
                }.into_any(),
                OptionsMode::Sweep => view! {
                    <FieldRow>
                        <TextField
                            label="top-k values".to_string()
                            hint="e.g. 2,3,5,8".to_string()
                            value=sweep_top_k_input
                            set_value=set_sweep_top_k_input
                        />
                        <TextField
                            label="min-score values".to_string()
                            hint="milli, e.g. 0,500,800 or 0-800:200".to_string()
                            value=sweep_min_score_input
                            set_value=set_sweep_min_score_input
                        />
                    </FieldRow>
                }.into_any(),
            }}

            {move || match options_computed.get() {
                Err(e) => view! { <div class="text-sm log-line-error">{e}</div> }.into_any(),
                Ok(_) => ().into_any(),
            }}
        </div>
    }
}

#[component]
fn RunModePicker(
    run_mode: ReadSignal<RunMode>,
    set_run_mode: WriteSignal<RunMode>,
) -> impl IntoView {
    view! {
        <div class="space-y-2">
            <div class="eyebrow">"Run mode"</div>
            <div class="space-y-1.5">
                <RunModeOption
                    label="Score every combination"
                    body="Runs every variant × options pair. Best for finding the absolute winner across a small grid."
                    value=run_mode
                    set_value=set_run_mode
                    target=RunMode::ScoreAll
                />
                <RunModeOption
                    label="Autotune (tuning + holdout)"
                    body="Splits the dataset 70/30, picks a winner on the tuning split, scores it on the holdout. Best for large grids."
                    value=run_mode
                    set_value=set_run_mode
                    target=RunMode::Autotune
                />
            </div>
        </div>
    }
}

#[component]
fn RunModeOption(
    label: &'static str,
    body: &'static str,
    value: ReadSignal<RunMode>,
    set_value: WriteSignal<RunMode>,
    target: RunMode,
) -> impl IntoView {
    let active = move || value.get() == target;
    view! {
        <button
            type="button"
            class=move || format!(
                "w-full text-left rounded border p-3 transition-colors {}",
                if active() {
                    "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                } else {
                    "border-[var(--color-border)] hover:border-[var(--color-border-strong)]"
                }
            )
            on:click=move |_| set_value.set(target)
        >
            <div class="flex items-center gap-2">
                <span class=move || if active() {
                    "inline-block w-2 h-2 rounded-full bg-[var(--color-accent)]"
                } else {
                    "inline-block w-2 h-2 rounded-full border border-[var(--color-text-faint)]"
                }></span>
                <span class="text-text font-medium">{label}</span>
            </div>
            <p class="text-sm muted mt-1 ml-4">{body}</p>
        </button>
    }
}

#[component]
fn CostSummary(cost: Memo<Result<(usize, usize, usize), String>>) -> impl IntoView {
    view! {
        <div class="text-sm">
            {move || match cost.get() {
                Ok((v, o, total)) => view! {
                    <span>
                        <span class="font-mono">{v}</span>
                        " variants × "
                        <span class="font-mono">{o}</span>
                        " options = "
                        <span class="text-text font-mono">{total}</span>
                        " evaluations"
                    </span>
                }.into_any(),
                Err(_) => view! {
                    <span class="log-line-error">"Fix errors above to see cost"</span>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn ModeRadio<T>(
    value: ReadSignal<T>,
    set_value: WriteSignal<T>,
    options: Vec<(T, &'static str)>,
) -> impl IntoView
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    view! {
        <div class="flex gap-1.5 flex-wrap">
            {options.into_iter().map(|(target, label)| {
                let active = move || value.get() == target;
                view! {
                    <button
                        type="button"
                        class=move || format!(
                            "px-3 py-1.5 rounded border text-sm transition-colors {}",
                            if active() {
                                "border-[var(--color-accent)] text-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                            } else {
                                "border-[var(--color-border)] muted hover:text-text"
                            }
                        )
                        on:click=move |_| set_value.set(target)
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn StrategyPicker(
    value: ReadSignal<ChunkStrategy>,
    set_value: WriteSignal<ChunkStrategy>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2">
            <span class="eyebrow">"Strategy"</span>
            <ModeRadio<ChunkStrategy>
                value=value
                set_value=set_value
                options=vec![
                    (ChunkStrategy::Section, "section"),
                    (ChunkStrategy::Bert, "bert"),
                    (ChunkStrategy::Llm, "llm"),
                ]
            />
        </div>
    }
}

#[component]
fn FieldRow(children: Children) -> impl IntoView {
    view! { <div class="flex flex-wrap gap-3">{children()}</div> }
}

#[component]
fn NumField(
    label: String,
    value: ReadSignal<u32>,
    set_value: WriteSignal<u32>,
    #[prop(default = 0)] min: u32,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 min-w-32">
            <span class="eyebrow">{label}</span>
            <input
                class="input font-mono"
                type="number"
                min=min
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let v: u32 = event_target_value(&e).parse().unwrap_or(min);
                    set_value.set(v.max(min));
                }
            />
        </label>
    }
}

#[component]
fn TextField(
    label: String,
    hint: String,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 flex-1 min-w-48">
            <span class="eyebrow">{label}</span>
            <input
                class="input font-mono"
                type="text"
                placeholder=hint.clone()
                prop:value=move || value.get()
                on:input=move |e| set_value.set(event_target_value(&e))
            />
        </label>
    }
}

fn variants_summary(computed: Memo<Result<Vec<ChunkingVariant>, String>>) -> String {
    match computed.get() {
        Ok(list) => {
            if list.is_empty() {
                "no variants".to_string()
            } else if list.len() <= 4 {
                list.iter()
                    .map(|v| v.label.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                let first_three: Vec<_> = list.iter().take(3).map(|v| v.label.clone()).collect();
                format!("{} + {} more", first_three.join(", "), list.len() - 3)
            }
        }
        Err(e) => format!("⚠ {e}"),
    }
}

fn options_summary(computed: Memo<Result<Vec<EvaluationRunOptions>, String>>) -> String {
    match computed.get() {
        Ok(list) => match list.len() {
            0 => "no options".to_string(),
            1 => {
                let o = &list[0];
                format!(
                    "top-k {} · min-score {:.2}",
                    o.top_k,
                    o.min_score_milli as f32 / 1000.0,
                )
            }
            n => format!("{n} option sets"),
        },
        Err(e) => format!("⚠ {e}"),
    }
}

fn build_single_variant(
    strategy: ChunkStrategy,
    section: u32,
    bert_target: u32,
    bert_overlap: u32,
    llm_micro: u32,
) -> Result<ChunkingVariant, String> {
    Ok(match strategy {
        ChunkStrategy::Section => ChunkingVariant {
            label: format!("section:{section}"),
            config: ChunkingConfig::Section(SectionChunkingConfig {
                max_section_tokens: section,
            }),
        },
        ChunkStrategy::Bert => ChunkingVariant {
            label: format!("bert:{bert_target}/{bert_overlap}"),
            config: ChunkingConfig::Bert(BertChunkingConfig {
                target_tokens: bert_target,
                overlap_tokens: bert_overlap,
                min_tokens: 96,
            }),
        },
        ChunkStrategy::Llm => ChunkingVariant {
            label: format!("llm:{llm_micro}"),
            config: ChunkingConfig::Llm(LlmChunkingConfig {
                target_tokens: 384,
                micro_chunk_tokens: llm_micro,
                generation_model_id: Uuid::nil(),
            }),
        },
    })
}

fn default_sweep_variants(seeds: &[ChunkingConfigurationDto]) -> Vec<ChunkingVariant> {
    seeds
        .iter()
        .map(|cc| ChunkingVariant {
            label: cc.name.clone(),
            config: cc.config,
        })
        .collect()
}

fn template_variants(
    template: &SweepTemplateDto,
    configs: &[ChunkingConfigurationDto],
) -> Vec<ChunkingVariant> {
    template
        .members
        .iter()
        .filter_map(|id| {
            configs
                .iter()
                .find(|cc| cc.chunking_configuration_id == *id)
                .map(|cc| ChunkingVariant {
                    label: cc.name.clone(),
                    config: cc.config,
                })
        })
        .collect()
}

#[component]
fn SweepTemplatePicker(
    sweep_templates: StoredValue<Vec<SweepTemplateDto>>,
    selected: ReadSignal<Option<Uuid>>,
    set_selected: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    let templates = sweep_templates.with_value(|t| t.clone());
    if templates.is_empty() {
        return view! {
            <p class="text-sm muted">
                "No sweep templates defined. Falling back to every chunking configuration in the registry. Create a template on /chunking to scope the sweep."
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

#[cfg(feature = "hydrate")]
fn load_sweep_template_pref() -> Option<Uuid> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    let raw = storage
        .get_item(SWEEP_TEMPLATE_STORAGE_KEY)
        .ok()
        .flatten()?;
    Uuid::parse_str(&raw).ok()
}

#[cfg(not(feature = "hydrate"))]
fn load_sweep_template_pref() -> Option<Uuid> {
    None
}

#[cfg(feature = "hydrate")]
fn store_sweep_template_pref(id: Option<Uuid>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(storage) = window.local_storage().ok().flatten() else {
        return;
    };
    match id {
        Some(id) => {
            let _ = storage.set_item(SWEEP_TEMPLATE_STORAGE_KEY, &id.to_string());
        }
        None => {
            let _ = storage.remove_item(SWEEP_TEMPLATE_STORAGE_KEY);
        }
    }
}

#[cfg(not(feature = "hydrate"))]
fn store_sweep_template_pref(_: Option<Uuid>) {}

fn build_section_sweep(values: Vec<u32>) -> Vec<ChunkingVariant> {
    values
        .into_iter()
        .map(|t| ChunkingVariant {
            label: format!("section:{t}"),
            config: ChunkingConfig::Section(SectionChunkingConfig {
                max_section_tokens: t,
            }),
        })
        .collect()
}

fn build_bert_sweep(targets: &[u32], overlaps: &[u32]) -> Vec<ChunkingVariant> {
    let mut out = Vec::with_capacity(targets.len() * overlaps.len());
    for &t in targets {
        for &o in overlaps {
            out.push(ChunkingVariant {
                label: format!("bert:{t}/{o}"),
                config: ChunkingConfig::Bert(BertChunkingConfig {
                    target_tokens: t,
                    overlap_tokens: o,
                    min_tokens: 96,
                }),
            });
        }
    }
    out
}

fn build_llm_sweep(values: Vec<u32>) -> Vec<ChunkingVariant> {
    values
        .into_iter()
        .map(|micro| ChunkingVariant {
            label: format!("llm:{micro}"),
            config: ChunkingConfig::Llm(LlmChunkingConfig {
                target_tokens: 384,
                micro_chunk_tokens: micro,
                generation_model_id: Uuid::nil(),
            }),
        })
        .collect()
}
