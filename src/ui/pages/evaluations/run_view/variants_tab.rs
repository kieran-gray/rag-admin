use std::cmp::Ordering;

use leptos::prelude::*;

use crate::shared::contracts::EvaluationRunDto;
use crate::shared::{evaluation_score, EvaluationResultSplit, EvaluationVariantResult};
use crate::ui::components::primitives::{EmptyState, Surface};

use crate::ui::pages::evaluations::run_detail::{
    analysis_bucket_for, best_per_metric, category_breakdown, equivalence_class_members,
    primary_leader, reliability_advisor, row_key, summarise_retrieval, CategoryBreakdownPanel,
    EquivalenceClassPanel, MetricLegend, ParetoPanel, PromoteHandle, ReliabilityAdvisor,
    ReplicatePanel, RunSummary, VariantsSection,
};

#[component]
pub fn VariantsTabBody(run: EvaluationRunDto) -> impl IntoView {
    let scoring_weights = run.scoring_weights;
    let mut variants = run.variants;
    variants.sort_by(|a, b| {
        evaluation_score(&b.metrics)
            .partial_cmp(&evaluation_score(&a.metrics))
            .unwrap_or(Ordering::Equal)
    });

    let bests = best_per_metric(&variants);
    let leader = primary_leader(&variants).cloned();
    let leader_key = leader.as_ref().map(row_key);
    let leader_score = leader.as_ref().map(|v| evaluation_score(&v.metrics));
    let leader_ci_half = leader
        .as_ref()
        .map(|v| v.metrics.composite_ci_half_width())
        .filter(|h| *h > 0.0005);
    let leader_split = leader.as_ref().map(|v| v.split);
    let retrieval_summary = summarise_retrieval(&variants);

    let analysis_bucket = analysis_bucket_for(&variants);
    let tied = equivalence_class_members(&variants, analysis_bucket);
    let advisor = reliability_advisor(&variants, &tied, analysis_bucket);

    let variant_count = variants.len();

    let has_champion = variants
        .iter()
        .any(|v| v.split == EvaluationResultSplit::Holdout);
    let run_uuid = run.run_id;
    let promote = PromoteHandle::new(run_uuid);
    let promote_status = promote.status;

    view! {
        <div class="space-y-6">
            <RunSummary
                leader_score=leader_score
                leader_ci_half=leader_ci_half
                leader_split=leader_split
                variants_count=variant_count
                retrieval=retrieval_summary
                weights=scoring_weights
            />

            {(!advisor.is_empty()).then(|| view! { <ReliabilityAdvisor entries=advisor /> })}

            {has_champion.then(|| view! { <ReplicatePanel run_id=run_uuid /> })}

            {(tied.len() > 1).then(|| view! { <EquivalenceClassPanel members=tied bucket=analysis_bucket /> })}

            {primary_leader(&variants).and_then(|leader| {
                let rows = category_breakdown(leader);
                (rows.len() > 1).then(|| view! { <CategoryBreakdownPanel rows=rows /> })
            })}

            {{
                let pareto: Vec<EvaluationVariantResult> = variants
                    .iter()
                    .filter(|v| Some(v.split) == analysis_bucket)
                    .cloned()
                    .collect();
                (pareto.len() > 1).then(|| view! { <ParetoPanel variants=pareto bucket=analysis_bucket /> })
            }}

            <MetricLegend />

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
                    <VariantsSection
                        run_id=run_uuid
                        variants=variants
                        bests=bests
                        leader_key=leader_key
                        promote=promote
                        promote_status=promote_status
                    />
                }.into_any()
            }}
        </div>
    }
}
