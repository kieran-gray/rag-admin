mod options;
mod sweeps;
mod variants;
mod widgets;

use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{
    ChunkingConfigurationDto, RunEvaluationRequestDto, SweepTemplateDto,
};
use crate::shared::reference_data::ChunkStrategy;
use crate::shared::{ChunkingVariant, EvaluationRunOptions};
use crate::ui::components::primitives::Surface;

use super::eval_parser::parse_u32_values;

use self::options::{options_summary, variants_summary, CostSummary, OptionsMode, OptionsPicker};
use self::sweeps::{
    build_bert_sweep, build_darn_sweep, build_llm_sweep, build_section_sweep, build_single_variant,
    default_sweep_variants, load_sweep_template_pref, store_sweep_template_pref, template_variants,
};
use self::variants::{VariantsMode, VariantsPicker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preset {
    ChunkingGrid,
    RetrievalGrid,
    FullGrid,
    SingleVariant,
}

impl Preset {
    fn label(self) -> &'static str {
        match self {
            Self::ChunkingGrid => "Grid: chunking",
            Self::RetrievalGrid => "Grid: retrieval",
            Self::FullGrid => "Full grid",
            Self::SingleVariant => "Single variant",
        }
    }
}

#[derive(Clone, Copy)]
pub struct LauncherCallbacks {
    pub on_start: Callback<RunEvaluationRequestDto>,
}

#[component]
pub fn EvaluationLauncher(
    chunking_configurations: StoredValue<Vec<ChunkingConfigurationDto>>,
    sweep_templates: StoredValue<Vec<SweepTemplateDto>>,
    active_dataset: ReadSignal<Option<Uuid>>,
    active_index_profile: ReadSignal<Option<Uuid>>,
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
    let (darn_size, set_darn_size) = signal(500u32);
    let (darn_overlap, set_darn_overlap) = signal(50u32);

    let (sweep_strategy, set_sweep_strategy) = signal(ChunkStrategy::Section);
    let (sweep_section_tokens_input, set_sweep_section_tokens_input) =
        signal("256,384,480,512".to_string());
    let (sweep_bert_targets_input, set_sweep_bert_targets_input) =
        signal("256,320,384,448".to_string());
    let (sweep_bert_overlaps_input, set_sweep_bert_overlaps_input) = signal("0,48,64".to_string());
    let (sweep_llm_micro_input, set_sweep_llm_micro_input) = signal("64,96,128".to_string());
    let (sweep_darn_sizes_input, set_sweep_darn_sizes_input) =
        signal("300,500,800,1000".to_string());

    let (custom_variants, set_custom_variants) = signal::<Vec<ChunkingVariant>>(Vec::new());

    let active_gen_model: Signal<Uuid> = Signal::derive(Uuid::nil);

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
    let (sweep_top_k_input, set_sweep_top_k_input) = signal("5,6,7,8".to_string());
    let (sweep_min_score_input, set_sweep_min_score_input) = signal("500,600,700,800".to_string());

    let apply_preset = move |p: Preset| match p {
        Preset::ChunkingGrid => {
            set_variants_mode.set(VariantsMode::SweepTemplate);
            set_options_mode.set(OptionsMode::Single);
        }
        Preset::RetrievalGrid => {
            set_variants_mode.set(VariantsMode::Single);
            set_options_mode.set(OptionsMode::Sweep);
        }
        Preset::FullGrid => {
            set_variants_mode.set(VariantsMode::SweepTemplate);
            set_options_mode.set(OptionsMode::Sweep);
        }
        Preset::SingleVariant => {
            set_variants_mode.set(VariantsMode::Single);
            set_options_mode.set(OptionsMode::Single);
        }
    };

    let variants_computed = Memo::new(move |_| match variants_mode.get() {
        VariantsMode::Single => build_single_variant(
            single_strategy.get(),
            section_tokens.get(),
            bert_target.get(),
            bert_overlap.get(),
            llm_micro.get(),
            darn_size.get(),
            darn_overlap.get(),
            active_gen_model.get(),
        )
        .map(|v| vec![v])
        .map_err(|e| format!("variant error: {e}")),
        VariantsMode::SweepTemplate => {
            let templates = sweep_templates.with_value(Clone::clone);
            let configs = chunking_configurations.with_value(Clone::clone);
            if templates.is_empty() {
                let seeded = default_sweep_variants(&configs);
                if seeded.is_empty() {
                    Err("No sweep templates or chunking configurations in the registry. Create some on /configuration/chunking.".into())
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
                        "Sweep template '{}' has no resolvable chunking configurations. Edit it on /configuration/chunking.",
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
                .map(|values| build_llm_sweep(values, active_gen_model.get()))
                .map_err(|e| format!("llm sweep: {e}")),
            ChunkStrategy::Darn => parse_u32_values(&sweep_darn_sizes_input.get(), 1, 8192, 100)
                .map(|values| build_darn_sweep(values, darn_overlap.get()))
                .map_err(|e| format!("darn sweep: {e}")),
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
        let Some(index_profile_id) = active_index_profile.get() else {
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

        callbacks.on_start.run(RunEvaluationRequestDto {
            dataset_id,
            index_profile_id,
            retrieval_profile_id: None,
            variants,
            options,
        });
    };

    let can_start = move || {
        !running.get()
            && active_dataset.get().is_some()
            && active_index_profile.get().is_some()
            && cost_summary.with(Result::is_ok)
    };

    view! {
        <Surface title="Score specific variants".to_string()>
            <p class="text-sm muted mb-4">
                "Explicit grid of chunking variants × retrieval options. Use this to compare a hand-picked set against a baseline, or to reproduce a prior run; the optimizer above is the right choice when you want to find the best."
            </p>
            <div class="space-y-5">
                <div class="flex items-center gap-2 flex-wrap">
                    <span class="eyebrow shrink-0">"Presets"</span>
                    {[Preset::ChunkingGrid, Preset::RetrievalGrid, Preset::FullGrid, Preset::SingleVariant]
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
                        darn_size=darn_size
                        set_darn_size=set_darn_size
                        darn_overlap=darn_overlap
                        set_darn_overlap=set_darn_overlap
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
                        sweep_darn_sizes_input=sweep_darn_sizes_input
                        set_sweep_darn_sizes_input=set_sweep_darn_sizes_input
                        custom_variants=custom_variants
                        set_custom_variants=set_custom_variants
                        sweep_templates=sweep_templates
                        selected_sweep_template=selected_sweep_template
                        set_selected_sweep_template=set_selected_sweep_template
                        variants_computed=variants_computed
                        active_gen_model=active_gen_model
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
                        } else if active_index_profile.get().is_none() {
                            "Select an index profile"
                        } else if cost_summary.with(Result::is_err) {
                            "Fix errors above"
                        } else {
                            "Score variants"
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
