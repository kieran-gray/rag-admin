use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::contracts::{aggregate_type, EvaluationRunDto};
use crate::core::{evaluation_score, EvaluationResultSplit, EvaluationVariantResult};
use crate::server_functions::evaluation::get_run;
use crate::ui::components::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};

mod advisor;
mod category;
mod equivalence;
mod pareto;
mod promote;
mod shared;
mod summary;
mod variants;

use advisor::{
    analysis_bucket_for, equivalence_class_members, reliability_advisor, ReliabilityAdvisor,
};
use category::{category_breakdown, CategoryBreakdownPanel};
use equivalence::EquivalenceClassPanel;
use pareto::ParetoPanel;
use promote::{PromoteHandle, ReplicatePanel};
use shared::{best_per_metric, primary_leader, row_key};
use summary::{summarise_retrieval, MetricLegend, RunSummary};
use variants::VariantsSection;

#[component]
pub fn RunDetailPage() -> impl IntoView {
    let params = use_params_map();
    let run_id = Memo::new(move |_| {
        params
            .with(|p| p.get("run_id").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });

    let run_invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let run = Resource::new(
        move || (run_id.get(), run_invalidator.get()),
        move |(id, _)| async move {
            match id {
                Some(id) => get_run(id).await.map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    view! {
        <Transition fallback=|| view! { <p class="muted">"Loading run…"</p> }>
            {move || run.get().map(|res| match res {
                Err(e) => view! {
                    <Surface><div class="log-line-error">{format!("Failed to load: {e}")}</div></Surface>
                }.into_any(),
                Ok(None) => view! {
                    <Surface>
                        <EmptyState
                            title="Run not found"
                            body="This run id is unknown or has been removed.".to_string()
                        />
                    </Surface>
                }.into_any(),
                Ok(Some(r)) => view! { <RunView run=r /> }.into_any(),
            })}
        </Transition>
    }
}

#[component]
fn RunView(run: EvaluationRunDto) -> impl IntoView {
    let (status_kind, status_label) = match run.status.as_str() {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "running" => (Status::Pending, "Running"),
        _ => (Status::Neutral, "Unknown"),
    };
    let short = run.run_id.to_string().chars().take(8).collect::<String>();

    let mut variants = run.variants;
    variants.sort_by(|a, b| {
        evaluation_score(&b.metrics)
            .partial_cmp(&evaluation_score(&a.metrics))
            .unwrap_or(std::cmp::Ordering::Equal)
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
    let created_at = run.created_at.clone();

    let has_champion = variants
        .iter()
        .any(|v| v.split == EvaluationResultSplit::Holdout);
    let run_uuid = run.run_id;
    let promote = PromoteHandle::new(run_uuid);
    let promote_status = promote.status;

    view! {
        <div>
            <PageHeader
                title=format!("run-{short}")
                eyebrow="Evaluations / Run".to_string()
                subtitle=format!("{variant_count} variants · {created_at}")
                actions=Box::new(move || view! {
                    <StatusPill label=status_label.to_string() kind=status_kind />
                }.into_any())
            />

            <div class="mb-4">
                <A href="/evaluations" attr:class="muted text-sm">"← Back to evaluations"</A>
            </div>

            <div class="space-y-6">
                <RunSummary
                    leader_score=leader_score
                    leader_ci_half=leader_ci_half
                    leader_split=leader_split
                    variants_count=variant_count
                    retrieval=retrieval_summary
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
                            variants=variants
                            bests=bests
                            leader_key=leader_key
                            promote=promote
                            promote_status=promote_status
                        />
                    }.into_any()
                }}
            </div>
        </div>
    }
}
