use leptos::prelude::*;

use crate::shared::{ChunkingVariant, EvaluationRunOptions};

use super::widgets::{FieldRow, ModeRadio, NumField, TextField};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OptionsMode {
    Single,
    Sweep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunMode {
    ScoreAll,
    Autotune,
}

#[allow(clippy::too_many_arguments)]
#[component]
pub(super) fn OptionsPicker(
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
                            hint="e.g. 2,3,5,8"
                            value=sweep_top_k_input
                            set_value=set_sweep_top_k_input
                        />
                        <TextField
                            label="min-score values".to_string()
                            hint="milli, e.g. 0,500,800 or 0-800:200"
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
pub(super) fn RunModePicker(
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
pub(super) fn CostSummary(cost: Memo<Result<(usize, usize, usize), String>>) -> impl IntoView {
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

pub(super) fn variants_summary(computed: Memo<Result<Vec<ChunkingVariant>, String>>) -> String {
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

pub(super) fn options_summary(computed: Memo<Result<Vec<EvaluationRunOptions>, String>>) -> String {
    match computed.get() {
        Ok(list) => match list.as_slice() {
            [] => "no options".to_string(),
            [o] => format!(
                "top-k {} · min-score {:.2}",
                o.top_k,
                o.min_score_milli as f32 / 1000.0,
            ),
            many => format!("{} option sets", many.len()),
        },
        Err(e) => format!("⚠ {e}"),
    }
}
