use std::cmp::Ordering;
use std::collections::HashSet;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::shared::contracts::EvaluationRunDto;
use crate::shared::{evaluation_score, EvaluationResultSplit, EvaluationVariantResult};
use crate::ui::components::primitives::{EmptyState, Surface};

use crate::ui::pages::workflows::run_detail::{
    analysis_bucket_for, best_per_metric, category_breakdown, equivalence_class_members,
    primary_leader, reliability_advisor, row_key, variants_pareto_keys, CategoryBreakdownPanel,
    DrillRequest, MetricLegend, ParetoPanel, PromoteHandle, ReplicatePanel, RunScoreData,
    RunScoreStrip, VariantFilterContext, VariantsSection,
};

use super::drilldown_modal::{DrilldownModal, ModalTarget};
use super::optimizer_section::{optimizer_section_available, OptimizerSection};

#[component]
pub fn RunSinglePage(run: EvaluationRunDto) -> impl IntoView {
    let scoring_weights = run.scoring_weights;
    let run_kind = run.kind;
    let run_uuid = run.run_id;
    let is_running = matches!(run.status.as_str(), "running" | "pending");

    let mut variants = run.variants;
    variants.sort_by(|a, b| {
        evaluation_score(&b.metrics)
            .partial_cmp(&evaluation_score(&a.metrics))
            .unwrap_or(Ordering::Equal)
    });

    let bests = best_per_metric(&variants);
    let leader = primary_leader(&variants).cloned();
    let leader_key = leader.as_ref().map(row_key);
    let analysis_bucket = analysis_bucket_for(&variants);
    let tied = equivalence_class_members(&variants, analysis_bucket);
    let tied_count = tied.len();
    let tied_keys: HashSet<String> = tied.iter().map(row_key).collect();
    let advisor = reliability_advisor(&variants, analysis_bucket);
    let variant_count = variants.len();

    let has_champion = variants
        .iter()
        .any(|v| v.split == EvaluationResultSplit::Holdout);
    let promote = PromoteHandle::new(run_uuid);
    let promote_status = promote.status;

    let score_data = leader.as_ref().map(|l| RunScoreData {
        score: evaluation_score(&l.metrics),
        ci_half: Some(l.metrics.composite_ci_half_width()).filter(|h| *h > 0.0005),
        split: l.split,
        variant_label: l.variant.label.clone(),
        top_k: l.options.top_k,
        min_score: l.options.min_score(),
        question_count: l.question_results.len() as u32,
    });

    let pareto_set = variants_pareto_keys(&variants, analysis_bucket);

    let filter_ctx = VariantFilterContext {
        leader_key: leader_key.clone(),
        tied_keys,
        pareto_keys: pareto_set,
    };

    let (modal_target, set_modal_target) = signal::<Option<ModalTarget>>(None);
    let open_variant_key =
        Signal::derive(move || modal_target.with(|t| t.as_ref().map(|d| d.row_key.clone())));

    let on_open_modal = Callback::new(move |req: DrillRequest| {
        set_modal_target.set(Some(ModalTarget {
            row_key: req.row_key,
            variant_label: req.variant_label,
            split: req.split,
        }));
    });
    let on_close_modal = Callback::new(move |_| {
        set_modal_target.set(None);
    });

    let (highlight_request, set_highlight_request) = signal(0u32);
    let on_view_ties = Callback::new(move |_| {
        set_highlight_request.update(|v| *v += 1);
        scroll_to_variants_section();
    });

    let category_rows = leader.as_ref().map(category_breakdown).unwrap_or_default();
    let show_category = category_rows.len() > 1;

    let pareto_variants: Vec<EvaluationVariantResult> = variants
        .iter()
        .filter(|v| Some(v.split) == analysis_bucket)
        .cloned()
        .collect();
    let show_pareto = pareto_variants.len() > 1;

    let optimizer_available = optimizer_section_available(run_kind, &variants);
    let variants_for_optimizer = variants.clone();

    view! {
        <div class="run-single-page space-y-6">
            <RunScoreStrip
                score=score_data
                tied_count=tied_count
                advisor=advisor
                variants_count=variant_count
                weights=scoring_weights
                on_view_ties=on_view_ties
            />

            {has_champion.then(|| view! { <ReplicatePanel run_id=run_uuid /> })}

            {if variants.is_empty() {
                view! {
                    <Surface>
                        <EmptyState
                            title="No variants yet"
                            body="The run may still be in progress; variants land here as they're scored.".to_string()
                        />
                    </Surface>
                }.into_any()
            } else {
                view! {
                    <div id="variants-section">
                        <VariantsSection
                            variants=variants
                            bests=bests
                            filter_ctx=filter_ctx
                            promote=promote
                            promote_status=promote_status
                            on_open_drilldown=on_open_modal
                            open_variant_key=open_variant_key
                            highlight_request=highlight_request
                        />
                    </div>
                }.into_any()
            }}

            {show_category.then(|| view! {
                <details class="run-collapsible">
                    <summary class="run-collapsible-summary">
                        <span class="run-collapsible-chevron" aria-hidden="true">"▸"</span>
                        <span class="run-collapsible-label">"Per-category breakdown"</span>
                        <span class="muted text-xs">"Leader's recall / precision / IoU by question category"</span>
                    </summary>
                    <div class="run-collapsible-body">
                        <CategoryBreakdownPanel rows=category_rows />
                    </div>
                </details>
            })}

            {show_pareto.then(|| view! {
                <details class="run-collapsible">
                    <summary class="run-collapsible-summary">
                        <span class="run-collapsible-chevron" aria-hidden="true">"▸"</span>
                        <span class="run-collapsible-label">"Pareto frontier"</span>
                        <span class="muted text-xs">"Score vs. retrieved tokens"</span>
                    </summary>
                    <div class="run-collapsible-body">
                        <ParetoPanel variants=pareto_variants bucket=analysis_bucket />
                    </div>
                </details>
            })}

            {optimizer_available.then(|| view! {
                <OptimizerSection
                    variants=variants_for_optimizer
                    is_running=is_running
                    default_open=is_running
                />
            })}

            <MetricLegend />

            <DrilldownModal
                run_id=run_uuid
                target=modal_target
                on_close=on_close_modal
            />
        </div>
    }
}

fn scroll_to_variants_section() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if let Some(element) = document.get_element_by_id("variants-section") {
        if let Ok(elem) = element.dyn_into::<web_sys::HtmlElement>() {
            elem.scroll_into_view();
        }
    }
}
