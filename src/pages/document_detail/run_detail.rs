use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use uuid::Uuid;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{
    EmptyState, MetricBar, MetricKind, PageHeader, Status, StatusPill, Surface,
};
use crate::server_functions::evaluation::get_run;
use crate::shared::{
    aggregate_type, evaluation_score, EvaluationResultSplit, EvaluationRunDto,
    EvaluationVariantResult, ReliabilityFlag,
};

fn variant_display(v: &EvaluationVariantResult) -> (String, Option<String>) {
    let config_label = v.variant.config.display_label();
    if v.variant.label == config_label {
        (config_label, None)
    } else {
        (config_label, Some(v.variant.label.clone()))
    }
}

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

#[derive(Clone, Copy)]
struct MetricDef {
    name: &'static str,
    short: &'static str,
    help: &'static str,
}

const METRIC_DEFS: &[MetricDef] = &[
    MetricDef {
        name: "Recall",
        short: "R",
        help: "Fraction of each question's reference span that the retrieved chunks cover. \
               1.0 means every byte of every reference was returned. Penalised by missing content.",
    },
    MetricDef {
        name: "Precision",
        short: "P",
        help: "Fraction of the retrieved chunks' bytes that fall inside a reference span. \
               Penalised by retrieving extra non-relevant content alongside the answer.",
    },
    MetricDef {
        name: "IoU",
        short: "IoU",
        help: "Intersection-over-Union: overlap between retrieved and reference spans divided \
               by their union. A combined measure that punishes both missed and excess content.",
    },
    MetricDef {
        name: "Precision-ω",
        short: "Pω",
        help: "Precision over only the chunks whose spans touch a reference. \
               Isolates retrieval quality from chunk-boundary noise.",
    },
];

#[component]
fn RunView(run: EvaluationRunDto) -> impl IntoView {
    let (status_kind, status_label) = match run.status.as_str() {
        "completed" => (Status::Ok, "Completed"),
        "failed" => (Status::Fail, "Failed"),
        "running" => (Status::Pending, "Running"),
        _ => (Status::Neutral, "Unknown"),
    };
    let short = run.run_id.to_string()[..8].to_string();

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

fn analysis_bucket_for(variants: &[EvaluationVariantResult]) -> Option<EvaluationResultSplit> {
    let has_validation = variants
        .iter()
        .any(|v| v.split == EvaluationResultSplit::Validation);
    if has_validation {
        return Some(EvaluationResultSplit::Validation);
    }
    primary_leader(variants).map(|v| v.split)
}

fn equivalence_class_members(
    variants: &[EvaluationVariantResult],
    bucket: Option<EvaluationResultSplit>,
) -> Vec<EvaluationVariantResult> {
    let Some(bucket) = bucket else {
        return Vec::new();
    };
    let Some(bucket_leader) = variants
        .iter()
        .filter(|v| v.split == bucket)
        .max_by(|a, b| {
            evaluation_score(&a.metrics)
                .partial_cmp(&evaluation_score(&b.metrics))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        return Vec::new();
    };
    let (leader_lo, leader_hi) = composite_ci_bounds(bucket_leader);

    let mut out: Vec<EvaluationVariantResult> = variants
        .iter()
        .filter(|v| v.split == bucket)
        .filter(|v| {
            if row_key(v) == row_key(bucket_leader) {
                return true;
            }
            let (lo, hi) = composite_ci_bounds(v);
            leader_hi.min(hi) >= leader_lo.max(lo)
        })
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        evaluation_score(&b.metrics)
            .partial_cmp(&evaluation_score(&a.metrics))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

fn composite_ci_bounds(variant: &EvaluationVariantResult) -> (f32, f32) {
    let low = variant.metrics.composite_ci_low;
    let high = variant.metrics.composite_ci_high;
    if high > low {
        (low, high)
    } else {
        let s = evaluation_score(&variant.metrics);
        (s, s)
    }
}

#[derive(Clone)]
struct AdvisorEntry {
    flag: ReliabilityFlag,
    detail: String,
}

const SMALL_SAMPLE_THRESHOLD: u32 = 25;

const WINNERS_CURSE_TRIAL_THRESHOLD: usize = 16;

fn reliability_advisor(
    variants: &[EvaluationVariantResult],
    tied: &[EvaluationVariantResult],
    analysis_bucket: Option<EvaluationResultSplit>,
) -> Vec<AdvisorEntry> {
    let mut out = Vec::new();

    for holdout in variants
        .iter()
        .filter(|v| v.split == EvaluationResultSplit::Holdout)
    {
        let holdout_trial = extract_trial_id(&holdout.variant.label);
        let selection = variants
            .iter()
            .filter(|v| {
                v.split == EvaluationResultSplit::Validation
                    || v.split == EvaluationResultSplit::Tuning
            })
            .find(|v| match holdout_trial {
                Some(h_tid) => extract_trial_id(&v.variant.label) == Some(h_tid),
                None => v.variant.label == holdout.variant.label && v.options == holdout.options,
            });
        let Some(selection) = selection else {
            continue;
        };
        let h_score = evaluation_score(&holdout.metrics);
        let s_score = evaluation_score(&selection.metrics);
        let gap = (h_score - s_score).abs();
        let threshold = holdout.metrics.composite_ci_half_width().max(0.005);
        if gap > threshold {
            out.push(AdvisorEntry {
                flag: ReliabilityFlag::ValidationHoldoutGap,
                detail: format!(
                    "'{}' scored {:.1}% on {} but {:.1}% on holdout (gap {:.1}pp, holdout 95% confidence interval half-width {:.1}pp). The selection may have overfit.",
                    holdout.variant.label,
                    s_score * 100.0,
                    selection.split.as_str(),
                    h_score * 100.0,
                    gap * 100.0,
                    threshold * 100.0,
                ),
            });
            break;
        }
    }

    if let Some(leader) = primary_leader(variants) {
        if let Some(n) = leader_question_count(leader) {
            if n > 0 && n < SMALL_SAMPLE_THRESHOLD {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::SmallSample,
                    detail: format!(
                        "Only {n} questions in the {} set. The 95% confidence interval is ±{:.1}pp on the leader; scores within that band are not distinguishable.",
                        leader.split.as_str(),
                        leader.metrics.composite_ci_half_width() * 100.0,
                    ),
                });
            }
        }
    }

    if tied.len() > 1 {
        let bucket_label = analysis_bucket
            .map(|b| b.as_str())
            .unwrap_or("the analysis bucket");
        out.push(AdvisorEntry {
            flag: ReliabilityFlag::StatisticalTie,
            detail: format!(
                "{} configs have 95% confidence intervals that overlap the {bucket_label} leader's. Confidence-interval overlap is a heuristic, not a formal equivalence test, but the ranking between them is unlikely to be reliable. Prefer the cheapest or simplest.",
                tied.len(),
            ),
        });
    }

    if let Some(bucket) = analysis_bucket {
        let mut in_bucket: Vec<&EvaluationVariantResult> =
            variants.iter().filter(|v| v.split == bucket).collect();
        in_bucket.sort_by(|a, b| {
            evaluation_score(&b.metrics)
                .partial_cmp(&evaluation_score(&a.metrics))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let q_size = in_bucket.len().div_ceil(4).clamp(2, 12);
        if in_bucket.len() >= 4 {
            let bucket_leader = in_bucket[0];
            let top: Vec<&EvaluationVariantResult> = in_bucket.iter().take(q_size).copied().collect();
            if top.iter().all(|v| ci_overlaps(bucket_leader, v)) {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::FlatLandscape,
                    detail: format!(
                        "The top {} configs in the {} bucket all overlap in their 95% confidence intervals. The dataset isn't discriminating between them. Consider adding Reasoning or Trick category questions.",
                        top.len(),
                        bucket.as_str(),
                    ),
                });
            }
        }
    }

    let tuning_count = variants
        .iter()
        .filter(|v| v.split == EvaluationResultSplit::Tuning)
        .count();
    let trial_count = if tuning_count > 0 {
        let mut ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for v in variants
            .iter()
            .filter(|v| v.split == EvaluationResultSplit::Tuning)
        {
            if let Some(tid) = extract_trial_id(&v.variant.label) {
                ids.insert(tid);
            }
        }
        ids.len()
    } else {
        variants.len()
    };
    if trial_count >= WINNERS_CURSE_TRIAL_THRESHOLD {
        out.push(AdvisorEntry {
            flag: ReliabilityFlag::StatisticalTie,
            detail: format!(
                "{trial_count} configs were evaluated. The max-over-N estimator is biased upward (winner's curse), and 95% confidence intervals aren't multiple-comparison adjusted; the reported best score is more optimistic than a single fresh run on the same config would produce. Replicate with a new seed to sanity-check."
            ),
        });
    }

    if let Some(leader) = primary_leader(variants) {
        if !leader.question_results.is_empty() {
            let mut categories: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for q in &leader.question_results {
                categories.insert(q.category.clone());
            }
            let only_fact = categories.len() == 1
                && categories.iter().next().map(String::as_str) == Some("fact_retrieval");
            if only_fact {
                out.push(AdvisorEntry {
                    flag: ReliabilityFlag::FlatLandscape,
                    detail: "Synthetic dataset is fact-retrieval only. Generator-shaped, extractive benchmarks reward lexical overlap; results will overstate retrieval quality on broader user queries. Add Architecture, Reasoning, or Trick category questions for a more honest signal.".to_string(),
                });
            }
        }
    }

    out
}

fn extract_trial_id(label: &str) -> Option<u32> {
    let rest = label.strip_prefix("trial-")?;
    let head = rest.split('-').next().unwrap_or(rest);
    head.parse().ok()
}

fn leader_question_count(v: &EvaluationVariantResult) -> Option<u32> {
    if v.question_results.is_empty() {
        None
    } else {
        Some(v.question_results.len() as u32)
    }
}

fn ci_overlaps(a: &EvaluationVariantResult, b: &EvaluationVariantResult) -> bool {
    let (a_lo, a_hi) = composite_ci_bounds(a);
    let (b_lo, b_hi) = composite_ci_bounds(b);
    a_hi.min(b_hi) >= a_lo.max(b_lo)
}

#[component]
fn ReliabilityAdvisor(entries: Vec<AdvisorEntry>) -> impl IntoView {
    view! {
        <Surface title="Reliability advisor".to_string()>
            <div class="space-y-3">
                {entries.into_iter().map(|e| view! {
                    <div class="advisor-banner">
                        <div class="advisor-banner-headline">{e.flag.headline()}</div>
                        <div class="advisor-banner-detail muted">{e.detail}</div>
                    </div>
                }).collect_view()}
            </div>
        </Surface>
    }
}

struct CategoryRow {
    category: String,
    n: usize,
    recall: f32,
    precision: f32,
    iou: f32,
}

fn category_breakdown(leader: &EvaluationVariantResult) -> Vec<CategoryRow> {
    use std::collections::BTreeMap;
    if leader.question_results.is_empty() {
        return Vec::new();
    }
    let mut by_cat: BTreeMap<String, (usize, f32, f32, f32)> = BTreeMap::new();
    for q in &leader.question_results {
        let entry = by_cat.entry(q.category.clone()).or_insert((0, 0.0, 0.0, 0.0));
        entry.0 += 1;
        entry.1 += q.recall;
        entry.2 += q.precision;
        entry.3 += q.iou;
    }
    by_cat
        .into_iter()
        .map(|(category, (n, r, p, i))| {
            let n_f = n.max(1) as f32;
            CategoryRow {
                category,
                n,
                recall: r / n_f,
                precision: p / n_f,
                iou: i / n_f,
            }
        })
        .collect()
}

fn pretty_category(slug: &str) -> &'static str {
    match slug {
        "fact_retrieval" => "Fact retrieval",
        "architecture" => "Architecture",
        "reasoning" => "Reasoning",
        "code" => "Code",
        "trick" => "Trick",
        _ => "Other",
    }
}

#[component]
fn CategoryBreakdownPanel(rows: Vec<CategoryRow>) -> impl IntoView {
    view! {
        <Surface title="Per-category breakdown".to_string()>
            <div class="text-xs muted mb-3">
                "Leader's mean recall / precision / IoU split by question category. Big gaps between categories mean the dataset isn't uniformly hard, or that one category is leaking into the chunker via lexical overlap. Trick questions are designed to have no correct retrieval, so 0% on the Trick row is the intended outcome."
            </div>
            <table class="variants-table">
                <thead>
                    <tr>
                        <th>"Category"</th>
                        <th class="num">"N"</th>
                        <th class="num">"Recall"</th>
                        <th class="num">"Precision"</th>
                        <th class="num">"IoU"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|r| {
                        let is_trick = r.category == "trick";
                        let trick_cell = move |value: f32| -> leptos::prelude::AnyView {

                                view! {
                                    <td class="num">{format!("{:.1}%", value * 100.0)}</td>
                                }.into_any()
                            
                        };
                        view! {
                            <tr>
                                <td>
                                    {pretty_category(&r.category)}
                                    {is_trick.then(|| view! {
                                        <span
                                            class="pill pill-neutral text-[10px] ml-2"
                                            title="Trick questions test that the retriever does not invent a reference span. 0% recall on these is the intended outcome."
                                        >"target 0%"</span>
                                    })}
                                </td>
                                <td class="num">{r.n}</td>
                                {trick_cell(r.recall)}
                                {trick_cell(r.precision)}
                                {trick_cell(r.iou)}
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </Surface>
    }
}

#[derive(Clone, Copy)]
struct PromoteHandle {
    run_id: uuid::Uuid,
    busy_label: ReadSignal<Option<String>>,
    set_busy_label: WriteSignal<Option<String>>,
    status: ReadSignal<Option<Result<String, String>>>,
    set_status: WriteSignal<Option<Result<String, String>>>,
}

impl PromoteHandle {
    fn new(run_id: uuid::Uuid) -> Self {
        let (busy_label, set_busy_label) = signal::<Option<String>>(None);
        let (status, set_status) = signal::<Option<Result<String, String>>>(None);
        Self {
            run_id,
            busy_label,
            set_busy_label,
            status,
            set_status,
        }
    }

    fn save(self, variant_label: String) {
        use crate::server_functions::evaluation::promote_variant_to_chunking_config;
        let run_id = self.run_id;
        let label_for_busy = variant_label.clone();
        let label_for_message = variant_label.clone();
        self.set_busy_label.set(Some(label_for_busy));
        self.set_status.set(None);
        leptos::task::spawn_local(async move {
            let res =
                promote_variant_to_chunking_config(run_id, variant_label, String::new()).await;
            self.set_busy_label.set(None);
            match res {
                Ok(_) => self.set_status.set(Some(Ok(format!(
                    "Saved '{label_for_message}'. Find it under Chunking."
                )))),
                Err(e) => self.set_status.set(Some(Err(e.to_string()))),
            }
        });
    }
}

#[component]
fn VariantSaveButton(
    variant_label: String,
    is_leader: bool,
    promote: PromoteHandle,
    #[prop(optional)] compact: bool,
) -> impl IntoView {
    let label_for_save = variant_label.clone();
    let label_for_busy = variant_label.clone();
    let is_busy = move || {
        promote
            .busy_label
            .with(|b| b.as_deref() == Some(label_for_busy.as_str()))
    };
    let any_busy = move || promote.busy_label.with(|b| b.is_some());
    let class_base = if is_leader { "btn btn-primary" } else { "btn" };
    let class = if compact {
        format!("{class_base} btn-compact")
    } else {
        class_base.to_string()
    };
    let title = if is_leader {
        "Save this champion as a named chunking configuration."
    } else {
        "Save this variant's chunking config under Chunking."
    };
    view! {
        <button
            type="button"
            class=class
            title=title
            prop:disabled=move || any_busy()
            on:click=move |_| promote.save(label_for_save.clone())
        >
            {move || if is_busy() { "Saving…" } else { "Save" }}
        </button>
    }
}

#[component]
fn ReplicatePanel(run_id: uuid::Uuid) -> impl IntoView {
    use crate::server_functions::evaluation::replicate_optimization_run;
    let (replicating, set_replicating) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let on_replicate = move |_| {
        set_error.set(None);
        set_replicating.set(true);
        leptos::task::spawn_local(async move {
            match replicate_optimization_run(run_id).await {
                Ok(new_run_id) => {
                    let url = format!("/runs/{new_run_id}/replicate?with={run_id}");
                    let _ = leptos::web_sys::window()
                        .and_then(|w| w.location().set_href(&url).ok());
                }
                Err(e) => {
                    set_replicating.set(false);
                    set_error.set(Some(e.to_string()));
                }
            }
        });
    };

    view! {
        <Surface title="Sanity-check the champion".to_string()>
            <p class="text-xs muted mb-3">
                "Launch a sibling run with a fresh random seed. A matching champion is a noisy positive signal, not a formal confidence test. To save a chunking config, use the Save button on any row of the Variants table below."
            </p>
            <button
                type="button"
                class="btn"
                prop:disabled=move || replicating.get()
                on:click=on_replicate
            >
                {move || if replicating.get() { "Replicating…" } else { "Replicate (new seed)" }}
            </button>
            {move || error.get().map(|e| view! {
                <div class="log-line-error text-xs mt-2">{e}</div>
            })}
        </Surface>
    }
}

#[component]
fn ParetoPanel(
    variants: Vec<EvaluationVariantResult>,
    bucket: Option<EvaluationResultSplit>,
) -> impl IntoView {
    let bucket_label = bucket
        .map(|b| b.as_str())
        .unwrap_or("analysis");
    use crate::shared::{pareto_frontier, ParetoPoint};

    let points: Vec<ParetoPoint> = variants
        .iter()
        .map(|v| ParetoPoint {
            quality: evaluation_score(&v.metrics),
            cost: v.metrics.average_retrieved_tokens as f32,
        })
        .collect();
    let frontier_idx: std::collections::HashSet<usize> =
        pareto_frontier(&points).into_iter().collect();
    let champion_idx = variants
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            evaluation_score(&a.metrics)
                .partial_cmp(&evaluation_score(&b.metrics))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i);

    const W: f32 = 480.0;
    const H: f32 = 200.0;
    const PAD_L: f32 = 40.0;
    const PAD_R: f32 = 16.0;
    const PAD_T: f32 = 12.0;
    const PAD_B: f32 = 28.0;
    let max_cost = points.iter().map(|p| p.cost).fold(0.0_f32, f32::max).max(1.0);
    let to_x = move |cost: f32| {
        PAD_L + (W - PAD_L - PAD_R) * (cost / max_cost).clamp(0.0, 1.0)
    };
    let to_y = move |q: f32| PAD_T + (H - PAD_T - PAD_B) * (1.0 - q.clamp(0.0, 1.0));
    let view_box = format!("0 0 {W} {H}");

    view! {
        <Surface title=format!("Pareto frontier · {bucket_label}")>
            <div class="text-xs muted mb-3">
                "Quality (composite) vs. cost (mean retrieved tokens per question). Points outlined in accent are Pareto-optimal: nothing else gets you more quality for less cost. The ★ is the best-scoring config in this bucket. For optimization runs the bucket is the validation pass, where cheaper-but-tied alternatives to the champion live."
            </div>
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox=view_box
                style="width: 100%; max-width: 480px; height: auto;"
            >
                {[0.0_f32, 0.25, 0.5, 0.75, 1.0]
                    .iter()
                    .map(|q| {
                        let y = to_y(*q);
                        view! {
                            <line
                                x1=PAD_L
                                x2=W - PAD_R
                                y1=y
                                y2=y
                                stroke="var(--color-border)"
                                stroke-dasharray="2,4"
                            />
                        }
                    })
                    .collect_view()}
                <line
                    x1=PAD_L
                    x2=PAD_L
                    y1=PAD_T
                    y2=H - PAD_B
                    stroke="var(--color-border-strong)"
                />
                <line
                    x1=PAD_L
                    x2=W - PAD_R
                    y1=H - PAD_B
                    y2=H - PAD_B
                    stroke="var(--color-border-strong)"
                />
                {points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let cx = to_x(p.cost);
                        let cy = to_y(p.quality);
                        let on_frontier = frontier_idx.contains(&i);
                        let is_champion = champion_idx == Some(i);
                        let fill = if is_champion {
                            "var(--color-accent)"
                        } else if on_frontier {
                            "var(--color-accent-soft)"
                        } else {
                            "var(--color-text-muted)"
                        };
                        let stroke = if on_frontier || is_champion {
                            "var(--color-accent)"
                        } else {
                            "var(--color-border-strong)"
                        };
                        let r = if is_champion { 6.0 } else { 4.0 };
                        view! {
                            <circle
                                cx=cx
                                cy=cy
                                r=r
                                fill=fill
                                stroke=stroke
                                stroke-width="1.5"
                            />
                        }
                    })
                    .collect_view()}
                <text
                    x=PAD_L - 6.0
                    y=PAD_T + 6.0
                    font-size="10"
                    fill="var(--color-text-muted)"
                    text-anchor="end"
                >
                    "100%"
                </text>
                <text
                    x=PAD_L - 6.0
                    y=H - PAD_B + 3.0
                    font-size="10"
                    fill="var(--color-text-muted)"
                    text-anchor="end"
                >
                    "0%"
                </text>
                <text
                    x=W / 2.0
                    y=H - 6.0
                    font-size="10"
                    fill="var(--color-text-muted)"
                    text-anchor="middle"
                >
                    {format!("cost → (max ≈ {:.0})", max_cost)}
                </text>
            </svg>
        </Surface>
    }
}

#[component]
fn EquivalenceClassPanel(
    members: Vec<EvaluationVariantResult>,
    bucket: Option<EvaluationResultSplit>,
) -> impl IntoView {
    let count = members.len();
    let bucket_label = bucket.map(|b| b.as_str()).unwrap_or("analysis");
    view! {
        <Surface
            title=format!("Possible ties · {count} configs overlap on {bucket_label}")
        >
            <div class="text-xs muted mb-3">
                "These configs' composite-score 95% confidence intervals overlap the leader's in this bucket. Confidence-interval overlap is a rough heuristic, not a formal equivalence test, but the optimizer's ranking between them is unlikely to be reliable at this dataset size. Prefer the cheapest or simplest unless you have a reason."
            </div>
            <table class="variants-table">
                <thead>
                    <tr>
                        <th>"Variant"</th>
                        <th class="num">"TopK"</th>
                        <th class="num">"Min score"</th>
                        <th class="num">"Score"</th>
                        <th class="num" title="95% bootstrap confidence interval on the composite score">"95% confidence interval"</th>
                    </tr>
                </thead>
                <tbody>
                    {members.into_iter().enumerate().map(|(i, v)| {
                        let score = evaluation_score(&v.metrics);
                        let (lo, hi) = composite_ci_bounds(&v);
                        let ci_label = if hi > lo {
                            format!("{:.1}% – {:.1}%", lo * 100.0, hi * 100.0)
                        } else {
                            "—".into()
                        };
                        let (headline, trial_tag) = variant_display(&v);
                        view! {
                            <tr class=if i == 0 { "is-leader" } else { "" }>
                                <td>
                                    <span class="flex items-center gap-2">
                                        {(i == 0).then(|| view! {
                                            <span class="text-[var(--color-accent)]" title="Leader">"★"</span>
                                        })}
                                        <span class="font-mono">{headline}</span>
                                        {trial_tag.map(|t| view! {
                                            <span class="pill pill-neutral font-mono" title="Optimizer trial id">{t}</span>
                                        })}
                                    </span>
                                </td>
                                <td class="num">{v.options.top_k}</td>
                                <td class="num">{format!("{:.2}", v.options.min_score())}</td>
                                <td class="num"><strong>{format!("{:.1}%", score * 100.0)}</strong></td>
                                <td class="num muted">{ci_label}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </Surface>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantsView {
    Bars,
    Table,
}

#[component]
fn VariantsSection(
    variants: Vec<EvaluationVariantResult>,
    bests: MetricBests,
    leader_key: Option<String>,
    promote: PromoteHandle,
    promote_status: ReadSignal<Option<Result<String, String>>>,
) -> impl IntoView {
    let variants = StoredValue::new(variants);
    let leader_key = StoredValue::new(leader_key);
    let (mode, set_mode) = signal(VariantsView::Bars);

    let actions: Box<dyn Fn() -> leptos::prelude::AnyView + Send + Sync> = Box::new(move || {
        view! {
            <div class="seg-toggle" role="tablist" aria-label="Result view">
                <button
                    type="button"
                    role="tab"
                    class:is-active=move || mode.get() == VariantsView::Bars
                    aria-selected=move || (mode.get() == VariantsView::Bars).to_string()
                    on:click=move |_| set_mode.set(VariantsView::Bars)
                >"Bars"</button>
                <button
                    type="button"
                    role="tab"
                    class:is-active=move || mode.get() == VariantsView::Table
                    aria-selected=move || (mode.get() == VariantsView::Table).to_string()
                    on:click=move |_| set_mode.set(VariantsView::Table)
                >"Table"</button>
            </div>
        }
        .into_any()
    });

    let body = move || match mode.get() {
        VariantsView::Bars => view! {
            <div class="space-y-5">
                {variants.with_value(|vs| {
                    vs.iter().cloned().map(|v| {
                        let leader = leader_key.with_value(|l| l.as_deref() == Some(row_key(&v).as_str()));
                        view! {
                            <VariantCard variant=v leader=leader bests=bests promote=promote />
                        }
                    }).collect_view()
                })}
            </div>
            <AxisLegend />
        }
        .into_any(),
        VariantsView::Table => view! {
            <VariantTable
                variants=variants.with_value(|vs| vs.clone())
                bests=bests
                leader_key=leader_key.with_value(|k| k.clone())
                promote=promote
            />
        }
        .into_any(),
    };

    let status_banner = move || promote_status.get().map(|r| match r {
        Ok(msg) => view! {
            <div class="promote-status promote-status-ok mb-3">{msg}</div>
        }.into_any(),
        Err(e) => view! {
            <div class="promote-status promote-status-err mb-3">{e}</div>
        }.into_any(),
    });

    view! {
        <Surface title="Variants".to_string() actions=actions sticky_header=true>
            {status_banner}
            {body}
        </Surface>
    }
}

#[component]
fn RunSummary(
    leader_score: Option<f32>,
    leader_ci_half: Option<f32>,
    leader_split: Option<EvaluationResultSplit>,
    variants_count: usize,
    retrieval: RetrievalSummary,
) -> impl IntoView {
    let weights = crate::shared::EvaluationScoreWeights::default();
    let score_str = leader_score
        .map(|s| format!("{:.1}%", s * 100.0))
        .unwrap_or_else(|| "—".to_string());
    let ci_str = leader_ci_half.map(|h| format!(" ± {:.1}", h * 100.0));
    let eyebrow = match leader_split {
        Some(EvaluationResultSplit::Holdout) => "Leader score · holdout".to_string(),
        Some(EvaluationResultSplit::Validation) => "Leader score · validation".to_string(),
        Some(EvaluationResultSplit::Tuning) => "Leader score · tuning (preliminary)".to_string(),
        Some(EvaluationResultSplit::Full) => "Leader score".to_string(),
        None => "Leader score".to_string(),
    };
    let footnote = match leader_split {
        Some(EvaluationResultSplit::Holdout) => {
            "Final integrity pass on held-out questions, single touch.".to_string()
        }
        Some(EvaluationResultSplit::Validation) => {
            "Validation pass; chooses the champion before the holdout touch.".to_string()
        }
        Some(EvaluationResultSplit::Tuning) => {
            "Optimizer rung; preliminary, not the deployment number.".to_string()
        }
        _ => "Weighted composite ± 95% bootstrap confidence interval half-width".to_string(),
    };
    view! {
        <Surface flush=true>
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-px bg-[var(--color-border)]">
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Variants compared"</div>
                    <div class="text-lg mt-1 font-mono">{variants_count}</div>
                </div>
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">{eyebrow}</div>
                    <div class="text-lg mt-1 font-mono text-[var(--color-accent)]">
                        {score_str}
                        {ci_str.map(|s| view! { <span class="text-sm muted">{s}</span> })}
                    </div>
                    <div class="text-xs muted mt-1">{footnote}</div>
                </div>
                <div
                    class="bg-[var(--color-surface-1)] p-4"
                    title=retrieval.tooltip
                >
                    <div class="eyebrow">"Retrieval"</div>
                    <div class="text-lg mt-1 font-mono">{retrieval.headline}</div>
                    <div class="text-xs muted mt-1">{retrieval.detail}</div>
                </div>
                <div class="bg-[var(--color-surface-1)] p-4">
                    <div class="eyebrow">"Score weights"</div>
                    <div class="text-xs mt-1 muted font-mono">
                        {format!(
                            "Recall {:.0}% · IoU {:.0}% · Precision {:.0}% · Pω {:.0}%",
                            weights.recall * 100.0,
                            weights.iou * 100.0,
                            weights.precision * 100.0,
                            weights.precision_omega * 100.0,
                        )}
                    </div>
                </div>
            </div>
        </Surface>
    }
}

#[derive(Clone)]
struct RetrievalSummary {
    headline: String,
    detail: String,
    tooltip: String,
}

fn summarise_retrieval(variants: &[EvaluationVariantResult]) -> RetrievalSummary {
    if variants.is_empty() {
        return RetrievalSummary {
            headline: "—".into(),
            detail: "No variants".into(),
            tooltip: String::new(),
        };
    }

    let mut top_ks: Vec<u32> = variants.iter().map(|v| v.options.top_k).collect();
    top_ks.sort_unstable();
    top_ks.dedup();

    let mut min_scores_milli: Vec<u32> =
        variants.iter().map(|v| v.options.min_score_milli).collect();
    min_scores_milli.sort_unstable();
    min_scores_milli.dedup();

    let format_top_ks = |slice: &[u32]| {
        slice
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let format_min_scores = |slice: &[u32]| {
        slice
            .iter()
            .map(|m| format!("{:.2}", (*m as f32) / 1000.0))
            .collect::<Vec<_>>()
            .join(", ")
    };

    const INLINE_LIMIT: usize = 3;

    let top_k_part = match top_ks.as_slice() {
        [k] => format!("topK {k}"),
        ks if ks.len() <= INLINE_LIMIT => format!("topK {{{}}}", format_top_ks(ks)),
        ks => format!(
            "topK {} to {} ({} values)",
            ks.first().copied().unwrap_or(0),
            ks.last().copied().unwrap_or(0),
            ks.len(),
        ),
    };
    let min_part = match min_scores_milli.as_slice() {
        [m] => format!("min {:.2}", (*m as f32) / 1000.0),
        ms if ms.len() <= INLINE_LIMIT => format!("min {{{}}}", format_min_scores(ms)),
        ms => format!(
            "min {:.2} to {:.2} ({} values)",
            (*ms.first().unwrap_or(&0) as f32) / 1000.0,
            (*ms.last().unwrap_or(&0) as f32) / 1000.0,
            ms.len(),
        ),
    };

    let combinations = top_ks.len() * min_scores_milli.len();
    let detail = if combinations <= 1 {
        "Shared across all variants".to_string()
    } else {
        format!("Swept across {combinations} combinations")
    };

    let tooltip = if top_ks.len() > INLINE_LIMIT || min_scores_milli.len() > INLINE_LIMIT {
        format!(
            "topK: {}\nmin: {}",
            format_top_ks(&top_ks),
            format_min_scores(&min_scores_milli),
        )
    } else {
        String::new()
    };

    RetrievalSummary {
        headline: format!("{top_k_part} · {min_part}"),
        detail,
        tooltip,
    }
}

#[component]
fn MetricLegend() -> impl IntoView {
    view! {
        <div class="my-4">
            <details>
                <summary class="text-xs muted cursor-pointer hover:text-text">
                    "What do these metrics mean?"
                </summary>
                <div class="mt-3 p-4 surface-raised rounded text-sm space-y-2.5">
                    {METRIC_DEFS.iter().map(|d| view! {
                        <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline">
                            <span class="font-medium">{d.name}</span>
                            <span class="text-xs muted font-mono">{d.short}</span>
                            <span class="muted">{d.help}</span>
                        </div>
                    }).collect_view()}
                    <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline pt-2 border-t border-[var(--color-border)]">
                        <span class="font-medium">"± marker"</span>
                        <span class="text-xs muted font-mono">"95% CI"</span>
                        <span class="muted">
                            "95% bootstrap confidence interval half-width on the mean. A measure of statistical uncertainty given this dataset size. Two variants whose intervals overlap can't be ranked reliably at this dataset size."
                        </span>
                    </div>
                    <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline">
                        <span class="font-medium">"Pink tick"</span>
                        <span class="text-xs muted font-mono">"▎"</span>
                        <span class="muted">
                            "Best score for that metric across all variants in the run. Lets you see how close a runner-up is to the leader on each dimension."
                        </span>
                    </div>
                    <div class="grid grid-cols-[7rem_2rem_1fr] gap-3 items-baseline">
                        <span class="font-medium">"Judge†"</span>
                        <span class="text-xs muted font-mono">"qual"</span>
                        <span class="muted">
                            "Qualitative LLM-judge diagnostic across a 5-question sample. Useful as a sanity check on whether the retrieved context looks sufficient; the sample is too small to be a primary score and is not used by the optimizer."
                        </span>
                    </div>
                </div>
            </details>
        </div>
    }
}

#[component]
fn VariantCard(
    variant: EvaluationVariantResult,
    leader: bool,
    bests: MetricBests,
    promote: PromoteHandle,
) -> impl IntoView {
    let kind = if leader {
        MetricKind::Best
    } else {
        MetricKind::Default
    };
    let score = evaluation_score(&variant.metrics);
    let (headline, trial_tag) = variant_display(&variant);
    let split = variant.split.as_str().to_string();
    let chunk_count = variant.metrics.chunk_count;
    let avg_tokens = variant.metrics.average_chunk_tokens;
    let selected = variant.selected;
    let top_k = variant.options.top_k;
    let min_score = variant.options.min_score();
    let m = variant.metrics;
    let variant_label = variant.variant.label.clone();

    view! {
        <div class=move || format!(
            "surface-raised rounded p-4 {}",
            if leader { "border-l-2 border-l-[var(--color-accent)]" } else { "" }
        )>
            <div class="flex items-center justify-between mb-3 gap-3 flex-wrap">
                <div class="flex items-center gap-3 min-w-0">
                    {leader.then(|| view! { <span class="text-[var(--color-accent)]">"★"</span> })}
                    <span class="font-mono text-base truncate">{headline}</span>
                    {trial_tag.map(|t| view! {
                        <span class="pill pill-neutral font-mono" title="Optimizer trial id">{t}</span>
                    })}
                    <span class="text-xs muted">
                        {format!(
                            "topK {top_k} · min {min_score:.2} · {chunk_count} chunks · avg {avg_tokens} tok"
                        )}
                    </span>
                </div>
                <div class="flex items-center gap-3">
                    <span class="text-xs muted">{format!("split: {split}")}</span>
                    {selected.then(|| view! {
                        <span class="pill pill-ok">"selected"</span>
                    })}
                    <span class="font-mono text-sm">
                        <span class="muted text-xs mr-1">"score"</span>
                        {format!("{:.1}%", score * 100.0)}
                    </span>
                    <VariantSaveButton
                        variant_label=variant_label
                        is_leader=leader
                        promote=promote
                        compact=true
                    />
                </div>
            </div>
            <div class="space-y-2">
                <MetricBar
                    label="Recall"
                    short="R"
                    help=METRIC_DEFS[0].help.to_string()
                    value=m.recall_mean
                    stddev=ci_half(m.recall_ci_low, m.recall_ci_high)
                    best=bests.recall
                    kind=kind
                />
                <MetricBar
                    label="Precision"
                    short="P"
                    help=METRIC_DEFS[1].help.to_string()
                    value=m.precision_mean
                    stddev=ci_half(m.precision_ci_low, m.precision_ci_high)
                    best=bests.precision
                    kind=kind
                />
                <MetricBar
                    label="IoU"
                    short="IoU"
                    help=METRIC_DEFS[2].help.to_string()
                    value=m.iou_mean
                    stddev=ci_half(m.iou_ci_low, m.iou_ci_high)
                    best=bests.iou
                    kind=kind
                />
                <MetricBar
                    label="Precision-ω"
                    short="Pω"
                    help=METRIC_DEFS[3].help.to_string()
                    value=m.precision_omega_mean
                    stddev=ci_half(m.precision_omega_ci_low, m.precision_omega_ci_high)
                    best=bests.precision_omega
                    kind=kind
                />
            </div>
        </div>
    }
}

#[component]
fn AxisLegend() -> impl IntoView {
    view! {
        <div class="metric-bar-axis mt-4 pt-3 border-t border-[var(--color-border)]">
            <span></span>
            <div class="metric-bar-axis-scale">
                <span>"0%"</span>
                <span style="left: 25%">"25%"</span>
                <span style="left: 50%">"50%"</span>
                <span style="left: 75%">"75%"</span>
                <span style="left: 100%">"100%"</span>
            </div>
            <span></span>
        </div>
    }
}

#[derive(Clone, Copy, Default)]
struct MetricBests {
    recall: f32,
    precision: f32,
    iou: f32,
    precision_omega: f32,
}

fn best_per_metric(variants: &[EvaluationVariantResult]) -> MetricBests {
    let mut b = MetricBests::default();
    for v in variants {
        b.recall = b.recall.max(v.metrics.recall_mean);
        b.precision = b.precision.max(v.metrics.precision_mean);
        b.iou = b.iou.max(v.metrics.iou_mean);
        b.precision_omega = b.precision_omega.max(v.metrics.precision_omega_mean);
    }
    b
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Variant,
    Split,
    TopK,
    MinScore,
    Chunks,
    AvgTok,
    Recall,
    Precision,
    Iou,
    Pomega,
    Score,
    JudgeScore,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortDir {
    Asc,
    Desc,
}

impl SortKey {
    fn default_dir(self) -> SortDir {
        match self {
            SortKey::Variant | SortKey::Split => SortDir::Asc,
            _ => SortDir::Desc,
        }
    }
}

fn row_key(v: &EvaluationVariantResult) -> String {
    format!(
        "{}|{}|{}|{}",
        v.variant.label,
        v.split.as_str(),
        v.options.top_k,
        v.options.min_score_milli
    )
}

fn primary_leader(variants: &[EvaluationVariantResult]) -> Option<&EvaluationVariantResult> {
    const PRIORITY: [EvaluationResultSplit; 4] = [
        EvaluationResultSplit::Holdout,
        EvaluationResultSplit::Validation,
        EvaluationResultSplit::Full,
        EvaluationResultSplit::Tuning,
    ];
    let bucket = PRIORITY
        .iter()
        .copied()
        .find(|b| variants.iter().any(|v| v.split == *b))?;
    variants
        .iter()
        .filter(|v| v.split == bucket)
        .max_by(|a, b| {
            evaluation_score(&a.metrics)
                .partial_cmp(&evaluation_score(&b.metrics))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn cmp_variants(a: &EvaluationVariantResult, b: &EvaluationVariantResult, key: SortKey) -> std::cmp::Ordering {
    use std::cmp::Ordering::Equal;
    match key {
        SortKey::Variant => a.variant.label.cmp(&b.variant.label),
        SortKey::Split => a.split.as_str().cmp(b.split.as_str()),
        SortKey::TopK => a.options.top_k.cmp(&b.options.top_k),
        SortKey::MinScore => a.options.min_score_milli.cmp(&b.options.min_score_milli),
        SortKey::Chunks => a.metrics.chunk_count.cmp(&b.metrics.chunk_count),
        SortKey::AvgTok => a.metrics.average_chunk_tokens.cmp(&b.metrics.average_chunk_tokens),
        SortKey::Recall => a.metrics.recall_mean.partial_cmp(&b.metrics.recall_mean).unwrap_or(Equal),
        SortKey::Precision => a.metrics.precision_mean.partial_cmp(&b.metrics.precision_mean).unwrap_or(Equal),
        SortKey::Iou => a.metrics.iou_mean.partial_cmp(&b.metrics.iou_mean).unwrap_or(Equal),
        SortKey::Pomega => a.metrics.precision_omega_mean.partial_cmp(&b.metrics.precision_omega_mean).unwrap_or(Equal),
        SortKey::Score => evaluation_score(&a.metrics).partial_cmp(&evaluation_score(&b.metrics)).unwrap_or(Equal),
        SortKey::JudgeScore => a
            .metrics
            .judge_score
            .unwrap_or(-1.0)
            .partial_cmp(&b.metrics.judge_score.unwrap_or(-1.0))
            .unwrap_or(Equal),
    }
}

#[component]
fn VariantTable(
    variants: Vec<EvaluationVariantResult>,
    bests: MetricBests,
    leader_key: Option<String>,
    promote: PromoteHandle,
) -> impl IntoView {
    let leader_key = StoredValue::new(leader_key);
    let variants = StoredValue::new(variants);

    let (sort_key, set_sort_key) = signal(SortKey::Score);
    let (sort_dir, set_sort_dir) = signal(SortDir::Desc);

    let toggle = move |key: SortKey| {
        if sort_key.get_untracked() == key {
            set_sort_dir.update(|d| {
                *d = match *d {
                    SortDir::Asc => SortDir::Desc,
                    SortDir::Desc => SortDir::Asc,
                };
            });
        } else {
            set_sort_key.set(key);
            set_sort_dir.set(key.default_dir());
        }
    };

    let sorted = move || {
        let key = sort_key.get();
        let dir = sort_dir.get();
        let mut rows = variants.with_value(|vs| vs.clone());
        rows.sort_by(|a, b| {
            let ord = cmp_variants(a, b, key);
            match dir {
                SortDir::Asc => ord,
                SortDir::Desc => ord.reverse(),
            }
        });
        rows
    };

    view! {
        <div class="variants-table-scroll">
            <table class="variants-table">
                <thead>
                    <tr>
                        <th class="num">"#"</th>
                        <SortableTh label="Variant" key=SortKey::Variant numeric=false sort_key=sort_key sort_dir=sort_dir on_sort=toggle />
                        <SortableTh label="Split" key=SortKey::Split numeric=false sort_key=sort_key sort_dir=sort_dir on_sort=toggle />
                        <SortableTh label="TopK" key=SortKey::TopK numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle />
                        <SortableTh label="Min score" key=SortKey::MinScore numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle />
                        <SortableTh label="Chunks" key=SortKey::Chunks numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle />
                        <SortableTh label="Avg tok" key=SortKey::AvgTok numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle />
                        <SortableTh label="Recall" key=SortKey::Recall numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle title="Recall (mean ± 95% bootstrap confidence interval half-width)"/>
                        <SortableTh label="Precision" key=SortKey::Precision numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle title="Precision (mean ± 95% bootstrap confidence interval half-width)"/>
                        <SortableTh label="IoU" key=SortKey::Iou numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle title="Intersection-over-Union (mean ± 95% bootstrap confidence interval half-width)"/>
                        <SortableTh label="Pω" key=SortKey::Pomega numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle title="Precision-ω (mean ± 95% bootstrap confidence interval half-width)"/>
                        <SortableTh label="Score" key=SortKey::Score numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle title="Weighted composite score" />
                        <SortableTh label="Judge†" key=SortKey::JudgeScore numeric=true sort_key=sort_key sort_dir=sort_dir on_sort=toggle title="Qualitative diagnostic only. LLM judge across 5 sampled validation questions per survivor. Use as a sanity check, not as a primary score." />
                        <th class="num" title="Save this variant's chunking config under Chunking.">"Save"</th>
                    </tr>
                </thead>
                <tbody>
                    {move || sorted().into_iter().enumerate().map(|(i, v)| {
                        let key = row_key(&v);
                        let leader = leader_key.with_value(|l| l.as_deref() == Some(key.as_str()));
                        let score = evaluation_score(&v.metrics);
                        let m = v.metrics.clone();
                        let row_class = if leader { "is-leader" } else { "" };
                        let (headline, trial_tag) = variant_display(&v);
                        view! {
                            <tr class=row_class>
                                <td class="num muted">{i + 1}</td>
                                <td>
                                    <span class="flex items-center gap-2">
                                        {leader.then(|| view! {
                                            <span class="text-[var(--color-accent)]" title="Leader">"★"</span>
                                        })}
                                        <span class="font-mono">{headline}</span>
                                        {trial_tag.map(|t| view! {
                                            <span class="pill pill-neutral font-mono" title="Optimizer trial id">{t}</span>
                                        })}
                                        {v.selected.then(|| view! {
                                            <span class="pill pill-ok">"selected"</span>
                                        })}
                                    </span>
                                </td>
                                <td class="muted">{v.split.as_str()}</td>
                                <td class="num">{v.options.top_k}</td>
                                <td class="num">{format!("{:.2}", v.options.min_score())}</td>
                                <td class="num">{v.metrics.chunk_count}</td>
                                <td class="num">{v.metrics.average_chunk_tokens}</td>
                                {metric_cell(m.recall_mean, m.recall_ci_low, m.recall_ci_high, bests.recall)}
                                {metric_cell(m.precision_mean, m.precision_ci_low, m.precision_ci_high, bests.precision)}
                                {metric_cell(m.iou_mean, m.iou_ci_low, m.iou_ci_high, bests.iou)}
                                {metric_cell(m.precision_omega_mean, m.precision_omega_ci_low, m.precision_omega_ci_high, bests.precision_omega)}
                                <td class="num"><strong>{format!("{:.1}%", score * 100.0)}</strong></td>
                                <td class="num muted">{
                                    m.judge_score
                                        .map(|s| format!("{:.0}%", s * 100.0))
                                        .unwrap_or_else(|| "—".into())
                                }</td>
                                <td class="num">
                                    <VariantSaveButton
                                        variant_label=v.variant.label.clone()
                                        is_leader=leader
                                        promote=promote
                                        compact=true
                                    />
                                </td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn SortableTh(
    label: &'static str,
    key: SortKey,
    numeric: bool,
    sort_key: ReadSignal<SortKey>,
    sort_dir: ReadSignal<SortDir>,
    on_sort: impl Fn(SortKey) + Send + Sync + 'static,
    #[prop(optional)] title: &'static str,
) -> impl IntoView {
    let on_sort = StoredValue::new(on_sort);
    let is_active = move || sort_key.get() == key;
    let aria_sort = move || {
        if is_active() {
            match sort_dir.get() {
                SortDir::Asc => "ascending",
                SortDir::Desc => "descending",
            }
        } else {
            "none"
        }
    };
    let arrow = move || {
        if is_active() {
            match sort_dir.get() {
                SortDir::Asc => " ▲",
                SortDir::Desc => " ▼",
            }
        } else {
            ""
        }
    };
    let class = if numeric {
        "num sortable"
    } else {
        "sortable"
    };
    let title_attr = if title.is_empty() { label } else { title };
    view! {
        <th
            class=move || format!(
                "{class}{}",
                if is_active() { " is-sorted" } else { "" }
            )
            aria-sort=aria_sort
            title=title_attr
            on:click=move |_| on_sort.with_value(|f| f(key))
        >
            {label}
            <span class="sort-indicator">{arrow}</span>
        </th>
    }
}

fn ci_half(low: f32, high: f32) -> f32 {
    ((high - low) / 2.0).max(0.0)
}

fn metric_cell(value: f32, ci_low: f32, ci_high: f32, best: f32) -> impl IntoView {
    let is_best = (value - best).abs() < 0.0005 && best > 0.0;
    let cell_class = if is_best { "num cell-best" } else { "num" };
    let half = ((ci_high - ci_low) / 2.0).max(0.0);
    let ci_view = (half > 0.0005).then(|| {
        view! {
            <span class="cell-stddev" title="95% bootstrap confidence interval half-width">
                {format!(" ± {:.1}", half * 100.0)}
            </span>
        }
    });
    view! {
        <td class=cell_class>
            {format!("{:.1}%", value * 100.0)}
            {ci_view}
        </td>
    }
}
