use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{QueryHit, QueryResult};
use crate::ui::components::playground::{HitBadge, HitCard};
use crate::ui::components::primitives::Surface;
use crate::ui::pages::shared::{hit_display_title, hit_source_link};

#[derive(Clone)]
pub(super) struct ComparisonRun {
    pub query: String,
    pub profile_name: String,
    pub top_k: u32,
    pub result: QueryResult,
}

#[derive(Clone, Copy)]
enum CompareColumn {
    Baseline,
    Candidate,
}

#[derive(Clone, Copy)]
enum RankDelta {
    Up(usize),
    Down(usize),
    Same,
    NotInOther,
    Unknown,
}

struct CompareSummary {
    moved_up: usize,
    moved_down: usize,
    same: usize,
    new_count: usize,
    dropped: usize,
    jaccard: f32,
}

fn build_ranks(hits: &[QueryHit]) -> HashMap<Uuid, usize> {
    let mut m = HashMap::with_capacity(hits.len());
    for (i, hit) in hits.iter().enumerate() {
        if let Some(cid) = hit.chunk_id {
            m.entry(cid).or_insert(i + 1);
        }
    }
    m
}

fn compute_delta(
    this_rank: usize,
    hit: &QueryHit,
    other_ranks: &HashMap<Uuid, usize>,
) -> RankDelta {
    let Some(cid) = hit.chunk_id else {
        return RankDelta::Unknown;
    };
    match other_ranks.get(&cid) {
        Some(&other) if other > this_rank => RankDelta::Up(other - this_rank),
        Some(&other) if other < this_rank => RankDelta::Down(this_rank - other),
        Some(_) => RankDelta::Same,
        None => RankDelta::NotInOther,
    }
}

fn delta_badge(delta: RankDelta, col: CompareColumn) -> Option<HitBadge> {
    let (label, class): (String, &'static str) = match (delta, col) {
        (RankDelta::Up(n), _) => (format!("↑{n}"), "compare-delta compare-delta-up"),
        (RankDelta::Down(n), _) => (format!("↓{n}"), "compare-delta compare-delta-down"),
        (RankDelta::Same, _) => ("=".to_string(), "compare-delta compare-delta-same"),
        (RankDelta::NotInOther, CompareColumn::Baseline) => {
            ("OUT".to_string(), "compare-delta compare-delta-out")
        }
        (RankDelta::NotInOther, CompareColumn::Candidate) => {
            ("NEW".to_string(), "compare-delta compare-delta-new")
        }
        (RankDelta::Unknown, _) => return None,
    };
    Some(HitBadge {
        label,
        class: class.to_string(),
    })
}

fn compare_summary(baseline: &[QueryHit], candidate: &[QueryHit]) -> CompareSummary {
    let baseline_ids: HashSet<Uuid> = baseline.iter().filter_map(|h| h.chunk_id).collect();
    let candidate_ids: HashSet<Uuid> = candidate.iter().filter_map(|h| h.chunk_id).collect();
    let intersection = baseline_ids.intersection(&candidate_ids).count();
    let union = baseline_ids.union(&candidate_ids).count();
    #[allow(clippy::cast_precision_loss)]
    let jaccard = if union == 0 {
        1.0
    } else {
        intersection as f32 / union as f32
    };

    let b_ranks = build_ranks(baseline);
    let c_ranks = build_ranks(candidate);
    let mut moved_up = 0;
    let mut moved_down = 0;
    let mut same = 0;
    for cid in baseline_ids.intersection(&candidate_ids) {
        let Some(&b) = b_ranks.get(cid) else { continue };
        let Some(&c) = c_ranks.get(cid) else { continue };
        if c < b {
            moved_up += 1;
        } else if c > b {
            moved_down += 1;
        } else {
            same += 1;
        }
    }
    let new_count = candidate_ids.difference(&baseline_ids).count();
    let dropped = baseline_ids.difference(&candidate_ids).count();

    CompareSummary {
        moved_up,
        moved_down,
        same,
        new_count,
        dropped,
        jaccard,
    }
}

#[allow(clippy::needless_pass_by_value)]
#[component]
pub(super) fn CompareView(
    baseline: ComparisonRun,
    candidate_query: String,
    candidate_hits: Vec<QueryHit>,
    on_unpin: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let baseline_hits = baseline.result.hits.clone();
    let summary = compare_summary(&baseline_hits, &candidate_hits);

    let baseline_ranks = build_ranks(&baseline_hits);
    let candidate_ranks = build_ranks(&candidate_hits);

    let jaccard_pct = format!("{:.0}%", summary.jaccard * 100.0);
    let CompareSummary {
        moved_up,
        moved_down,
        same,
        new_count,
        dropped,
        jaccard: _,
    } = summary;

    let baseline_label = format!(
        "Baseline · {} · k={}",
        baseline.profile_name, baseline.top_k
    );
    let baseline_query = baseline.query.clone();
    let candidate_label = format!("Candidate · {} hits", candidate_hits.len());

    view! {
        <Surface
            title="Comparison".to_string()
            actions=Box::new(move || view! {
                <button
                    type="button"
                    class="btn btn-sm btn-ghost"
                    on:click=move |_| on_unpin()
                >
                    "Unpin baseline"
                </button>
            }.into_any())
        >
            <div class="playground-body">
                <div class="compare-summary">
                    <span class="compare-summary-stat compare-summary-up">{format!("↑ {moved_up}")}</span>
                    <span class="compare-summary-stat compare-summary-down">{format!("↓ {moved_down}")}</span>
                    <span class="compare-summary-stat">{format!("= {same}")}</span>
                    <span class="compare-summary-stat compare-summary-new">{format!("NEW {new_count}")}</span>
                    <span class="compare-summary-stat compare-summary-out">{format!("OUT {dropped}")}</span>
                    <span class="compare-summary-stat compare-summary-jaccard">
                        "Jaccard "{jaccard_pct}
                    </span>
                </div>

                <div class="compare-grid">
                    <CompareColumnView
                        column=CompareColumn::Baseline
                        title=baseline_label
                        query=baseline_query.clone()
                        hits=baseline_hits
                        other_ranks=candidate_ranks
                    />
                    <CompareColumnView
                        column=CompareColumn::Candidate
                        title=candidate_label
                        query=candidate_query
                        hits=candidate_hits
                        other_ranks=baseline_ranks
                    />
                </div>
            </div>
        </Surface>
    }
}

#[component]
fn CompareColumnView(
    column: CompareColumn,
    title: String,
    query: String,
    hits: Vec<QueryHit>,
    other_ranks: HashMap<Uuid, usize>,
) -> impl IntoView {
    let other_ranks = StoredValue::new(other_ranks);

    view! {
        <div class="compare-column">
            <header class="compare-column-head">
                <div class="compare-column-title">{title}</div>
                <div class="compare-column-query muted text-sm">{query.clone()}</div>
            </header>
            <div class="compare-column-hits">
                {hits.into_iter().enumerate().map(|(i, hit)| {
                    let rank = i + 1;
                    let delta = other_ranks.with_value(|r| compute_delta(rank, &hit, r));
                    let badge = delta_badge(delta, column);
                    let title = hit_display_title(&hit);
                    let source = hit_source_link(&hit);
                    let heading = hit.heading.clone();
                    let snippet = hit.snippet.clone();
                    let footer_actions: Option<Children> = source.map(|href| {
                        let href = href.clone();
                        Box::new(move || view! {
                            <a class="btn btn-ghost btn-sm" href=href>"Open document"</a>
                        }.into_any()) as Children
                    });
                    view! {
                        <HitCard
                            rank=rank
                            score=hit.score
                            title=title
                            heading=heading
                            snippet=snippet
                            query=Some(query.clone())
                            badge=badge
                            header_actions=None
                            footer_actions=footer_actions
                            details=None
                            highlighted=false
                            extra_class=Some("compare-hit".to_string())
                        />
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
