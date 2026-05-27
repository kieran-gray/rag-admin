mod advisor;
mod category;
mod pareto;
mod promote;
mod shared;
mod summary;
mod variants;

pub(crate) use advisor::{analysis_bucket_for, equivalence_class_members, reliability_advisor};
pub(crate) use category::{category_breakdown, CategoryBreakdownPanel};
pub(crate) use pareto::ParetoPanel;
pub(crate) use promote::{PromoteHandle, ReplicatePanel};
pub(crate) use shared::{best_per_metric, primary_leader, row_key};
pub(crate) use summary::{MetricLegend, RunScoreData, RunScoreStrip};
pub(crate) use variants::{
    variants_pareto_keys, DrillRequest, VariantFilterContext, VariantsSection,
};
