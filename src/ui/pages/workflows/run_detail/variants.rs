use std::cmp::Ordering;
use std::collections::HashSet;

use leptos::prelude::*;

use crate::shared::{
    evaluation_score, pareto_frontier, ChunkingConfig, EvaluationResultSplit,
    EvaluationVariantResult, ParetoPoint,
};
use crate::ui::components::primitives::{MetricBar, MetricKind, Surface};

use super::promote::{PromoteHandle, VariantSaveButton};
use super::shared::{
    ci_half, row_key, variant_display, MetricBests, IOU_DEF, PRECISION_DEF, PRECISION_OMEGA_DEF,
    RECALL_DEF,
};
use super::summary::AxisLegend;

#[derive(Clone)]
pub(crate) struct DrillRequest {
    pub row_key: String,
    pub variant_label: String,
    pub split: EvaluationResultSplit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantFilter {
    Highlights,
    All,
    ByFamily,
}

impl VariantFilter {
    fn label(self) -> &'static str {
        match self {
            VariantFilter::Highlights => "Highlights",
            VariantFilter::All => "All",
            VariantFilter::ByFamily => "By family",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantSort {
    Score,
    Recall,
    Precision,
    Cost,
}

impl VariantSort {
    fn label(self) -> &'static str {
        match self {
            VariantSort::Score => "Score",
            VariantSort::Recall => "Recall",
            VariantSort::Precision => "Precision",
            VariantSort::Cost => "Cost",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantBadge {
    Leader,
    Tied,
    Pareto,
    None,
}

impl VariantBadge {
    fn glyph(self) -> Option<&'static str> {
        match self {
            VariantBadge::Leader => Some("★"),
            VariantBadge::Tied => Some("≡"),
            VariantBadge::Pareto => Some("⬣"),
            VariantBadge::None => None,
        }
    }

    fn label(self) -> Option<&'static str> {
        match self {
            VariantBadge::Leader => Some("Leader"),
            VariantBadge::Tied => Some("Tied"),
            VariantBadge::Pareto => Some("Pareto"),
            VariantBadge::None => None,
        }
    }
}

fn chunker_family(config: &ChunkingConfig) -> &'static str {
    match config {
        ChunkingConfig::Bert(_) => "BERT",
        ChunkingConfig::Section(_) => "Section",
        ChunkingConfig::Llm(_) => "LLM",
        ChunkingConfig::Darn(_) => "DARN",
    }
}

pub(crate) fn variants_pareto_keys(
    variants: &[EvaluationVariantResult],
    bucket: Option<EvaluationResultSplit>,
) -> HashSet<String> {
    let Some(bucket) = bucket else {
        return HashSet::new();
    };
    let in_bucket: Vec<&EvaluationVariantResult> =
        variants.iter().filter(|v| v.split == bucket).collect();
    let points: Vec<ParetoPoint> = in_bucket
        .iter()
        .map(|v| ParetoPoint {
            quality: evaluation_score(&v.metrics),
            cost: v.metrics.average_retrieved_tokens as f32,
        })
        .collect();
    pareto_frontier(&points)
        .into_iter()
        .filter_map(|i| in_bucket.get(i).map(|v| row_key(v)))
        .collect()
}

pub(crate) struct VariantFilterContext {
    pub leader_key: Option<String>,
    pub tied_keys: HashSet<String>,
    pub pareto_keys: HashSet<String>,
}

#[component]
pub(crate) fn VariantsSection(
    variants: Vec<EvaluationVariantResult>,
    bests: MetricBests,
    filter_ctx: VariantFilterContext,
    promote: PromoteHandle,
    promote_status: ReadSignal<Option<Result<String, String>>>,
    on_open_drawer: Callback<DrillRequest>,
    open_variant_key: Signal<Option<String>>,
    highlight_request: ReadSignal<u32>,
) -> impl IntoView {
    let total = variants.len();
    let variants = StoredValue::new(variants);
    let leader_key = StoredValue::new(filter_ctx.leader_key);
    let tied_keys = StoredValue::new(filter_ctx.tied_keys);
    let pareto_keys = StoredValue::new(filter_ctx.pareto_keys);

    let (filter, set_filter) = signal(VariantFilter::Highlights);
    let (sort, set_sort) = signal(VariantSort::Score);

    Effect::new(move |_| {
        if highlight_request.get() > 0 {
            set_filter.set(VariantFilter::Highlights);
        }
    });

    let badge_for = move |v: &EvaluationVariantResult| -> VariantBadge {
        let key = row_key(v);
        let is_leader = leader_key.with_value(|l| l.as_deref() == Some(key.as_str()));
        if is_leader {
            return VariantBadge::Leader;
        }
        if tied_keys.with_value(|t| t.contains(&key)) {
            return VariantBadge::Tied;
        }
        if pareto_keys.with_value(|p| p.contains(&key)) {
            return VariantBadge::Pareto;
        }
        VariantBadge::None
    };

    let visible_rows = move || {
        let f = filter.get();
        let s = sort.get();
        let rows = variants.with_value(Clone::clone);

        let mut filtered: Vec<EvaluationVariantResult> = match f {
            VariantFilter::Highlights => rows
                .into_iter()
                .filter(|v| !matches!(badge_for(v), VariantBadge::None))
                .collect(),
            VariantFilter::All | VariantFilter::ByFamily => rows,
        };

        filtered.sort_by(|a, b| {
            let primary = match s {
                VariantSort::Score => evaluation_score(&b.metrics)
                    .partial_cmp(&evaluation_score(&a.metrics))
                    .unwrap_or(Ordering::Equal),
                VariantSort::Recall => b
                    .metrics
                    .recall_mean
                    .partial_cmp(&a.metrics.recall_mean)
                    .unwrap_or(Ordering::Equal),
                VariantSort::Precision => b
                    .metrics
                    .precision_mean
                    .partial_cmp(&a.metrics.precision_mean)
                    .unwrap_or(Ordering::Equal),
                VariantSort::Cost => a
                    .metrics
                    .average_retrieved_tokens
                    .cmp(&b.metrics.average_retrieved_tokens),
            };
            if primary != Ordering::Equal {
                return primary;
            }
            evaluation_score(&b.metrics)
                .partial_cmp(&evaluation_score(&a.metrics))
                .unwrap_or(Ordering::Equal)
        });

        filtered
    };

    let visible_count = Memo::new(move |_| visible_rows().len());

    let actions: Box<dyn Fn() -> leptos::prelude::AnyView + Send + Sync> = Box::new(move || {
        view! {
            <div class="variants-toolbar">
                <FilterDropdown filter=filter set_filter=set_filter />
                <SortDropdown sort=sort set_sort=set_sort />
                <button
                    type="button"
                    class="btn btn-compact"
                    title="Download all variants as CSV"
                    on:click=move |_| download_csv(&variants.with_value(Clone::clone))
                >
                    "Export CSV"
                </button>
            </div>
        }
        .into_any()
    });

    let status_banner = move || {
        promote_status.get().map(|r| match r {
            Ok(msg) => view! {
                <div class="promote-status promote-status-ok mb-3">{msg}</div>
            }
            .into_any(),
            Err(e) => view! {
                <div class="promote-status promote-status-err mb-3">{e}</div>
            }
            .into_any(),
        })
    };

    let summary_line = move || {
        let n = visible_count.get();
        let f = filter.get();
        match f {
            VariantFilter::Highlights => {
                format!("Showing {n} of {total}: leader + ties + Pareto frontier")
            }
            VariantFilter::All => format!("Showing all {total} variants"),
            VariantFilter::ByFamily => format!("Showing all {total} variants grouped by chunker"),
        }
    };

    let body = move || {
        let rows = visible_rows();
        if rows.is_empty() {
            return view! {
                <div class="muted text-sm p-4">"No variants match the current filter."</div>
            }
            .into_any();
        }
        match filter.get() {
            VariantFilter::ByFamily => view! {
                <FamilyGroupedBars
                    rows=rows
                    bests=bests
                    badge_for=badge_for
                    promote=promote
                    on_open_drawer=on_open_drawer
                    open_variant_key=open_variant_key
                />
            }
            .into_any(),
            _ => view! {
                <div class="variants-bars-list">
                    {rows.into_iter().map(|v| view! {
                        <VariantCard
                            variant=v.clone()
                            badge=badge_for(&v)
                            bests=bests
                            promote=promote
                            on_open_drawer=on_open_drawer
                            open_variant_key=open_variant_key
                        />
                    }).collect_view()}
                </div>
                <AxisLegend />
            }
            .into_any(),
        }
    };

    view! {
        <Surface title="Variants".to_string() actions=actions sticky_header=true>
            {status_banner}
            <div class="variants-summary muted text-xs mb-3">{summary_line}</div>
            {body}
        </Surface>
    }
}

#[component]
fn FilterDropdown(
    filter: ReadSignal<VariantFilter>,
    set_filter: WriteSignal<VariantFilter>,
) -> impl IntoView {
    let options = [
        VariantFilter::Highlights,
        VariantFilter::All,
        VariantFilter::ByFamily,
    ];
    view! {
        <label class="dropdown-label">
            <span class="dropdown-label-text muted text-xs">"Filter"</span>
            <select
                class="dropdown-select"
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    let next = match v.as_str() {
                        "all" => VariantFilter::All,
                        "family" => VariantFilter::ByFamily,
                        _ => VariantFilter::Highlights,
                    };
                    set_filter.set(next);
                }
            >
                {options.into_iter().map(|opt| {
                    let value = match opt {
                        VariantFilter::Highlights => "highlights",
                        VariantFilter::All => "all",
                        VariantFilter::ByFamily => "family",
                    };
                    let selected = move || filter.get() == opt;
                    view! {
                        <option value=value prop:selected=selected>{opt.label()}</option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn SortDropdown(
    sort: ReadSignal<VariantSort>,
    set_sort: WriteSignal<VariantSort>,
) -> impl IntoView {
    let options = [
        VariantSort::Score,
        VariantSort::Recall,
        VariantSort::Precision,
        VariantSort::Cost,
    ];
    view! {
        <label class="dropdown-label">
            <span class="dropdown-label-text muted text-xs">"Sort"</span>
            <select
                class="dropdown-select"
                on:change=move |ev| {
                    let v = event_target_value(&ev);
                    let next = match v.as_str() {
                        "recall" => VariantSort::Recall,
                        "precision" => VariantSort::Precision,
                        "cost" => VariantSort::Cost,
                        _ => VariantSort::Score,
                    };
                    set_sort.set(next);
                }
            >
                {options.into_iter().map(|opt| {
                    let value = match opt {
                        VariantSort::Score => "score",
                        VariantSort::Recall => "recall",
                        VariantSort::Precision => "precision",
                        VariantSort::Cost => "cost",
                    };
                    let selected = move || sort.get() == opt;
                    view! {
                        <option value=value prop:selected=selected>{opt.label()}</option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn FamilyGroupedBars(
    rows: Vec<EvaluationVariantResult>,
    bests: MetricBests,
    badge_for: impl Fn(&EvaluationVariantResult) -> VariantBadge + 'static + Send + Sync + Copy,
    promote: PromoteHandle,
    on_open_drawer: Callback<DrillRequest>,
    open_variant_key: Signal<Option<String>>,
) -> impl IntoView {
    use std::collections::BTreeMap;
    let mut by_family: BTreeMap<&'static str, Vec<EvaluationVariantResult>> = BTreeMap::new();
    for v in rows {
        by_family
            .entry(chunker_family(&v.variant.config))
            .or_default()
            .push(v);
    }
    view! {
        <div class="space-y-6">
            {by_family.into_iter().map(|(family, vs)| view! {
                <div class="variants-family-group">
                    <div class="variants-family-head">
                        <span class="font-mono">{family}</span>
                        <span class="muted text-xs">{format!(" · {} variants", vs.len())}</span>
                    </div>
                    <div class="variants-bars-list">
                        {vs.into_iter().map(|v| view! {
                            <VariantCard
                                variant=v.clone()
                                badge=badge_for(&v)
                                bests=bests
                                promote=promote
                                on_open_drawer=on_open_drawer
                                open_variant_key=open_variant_key
                            />
                        }).collect_view()}
                    </div>
                </div>
            }).collect_view()}
            <AxisLegend />
        </div>
    }
}

#[component]
fn VariantCard(
    variant: EvaluationVariantResult,
    badge: VariantBadge,
    bests: MetricBests,
    promote: PromoteHandle,
    on_open_drawer: Callback<DrillRequest>,
    open_variant_key: Signal<Option<String>>,
) -> impl IntoView {
    let kind = if matches!(badge, VariantBadge::Leader) {
        MetricKind::Best
    } else {
        MetricKind::Default
    };
    let key = row_key(&variant);
    let key_open = key.clone();
    let key_class = key.clone();
    let score = evaluation_score(&variant.metrics);
    let half = variant.metrics.composite_ci_half_width();
    let ci_label = (half > 0.0005).then(|| format!(" ± {:.1}pp", half * 100.0));
    let (headline, trial_tag) = variant_display(&variant);
    let split = variant.split.as_str().to_string();
    let chunk_count = variant.metrics.chunk_count;
    let avg_tokens = variant.metrics.average_chunk_tokens;
    let avg_retrieved = variant.metrics.average_retrieved_tokens;
    let selected = variant.selected;
    let top_k = variant.options.top_k;
    let min_score = variant.options.min_score();
    let m = variant.metrics.clone();
    let variant_label = variant.variant.label.clone();
    let variant_split = variant.split;
    let drill_label = variant_label.clone();
    let is_open = move || open_variant_key.with(|k| k.as_deref() == Some(key_open.as_str()));
    let badge_glyph = badge.glyph();
    let badge_label = badge.label();
    let badge_class = match badge {
        VariantBadge::Leader => "variant-badge variant-badge-leader",
        VariantBadge::Tied => "variant-badge variant-badge-tied",
        VariantBadge::Pareto => "variant-badge variant-badge-pareto",
        VariantBadge::None => "variant-badge",
    };

    view! {
        <div
            class=move || format!(
                "variant-card {} {}",
                match badge {
                    VariantBadge::Leader => "is-leader",
                    VariantBadge::Tied => "is-tied",
                    VariantBadge::Pareto => "is-pareto",
                    VariantBadge::None => "",
                },
                if is_open() { "is-open" } else { "" }
            )
            data-variant-key=key_class.clone()
            on:click={
                let drill_label = drill_label.clone();
                let row_key = key.clone();
                move |_| {
                    on_open_drawer.run(DrillRequest {
                        row_key: row_key.clone(),
                        variant_label: drill_label.clone(),
                        split: variant_split,
                    });
                }
            }
        >
            <div class="variant-card-head">
                <div class="variant-card-title">
                    {badge_glyph.map(|g| view! {
                        <span class=badge_class title=badge_label.unwrap_or("")>{g}</span>
                    })}
                    <span class="font-mono">{headline}</span>
                    {trial_tag.map(|t| view! {
                        <span class="pill pill-neutral font-mono" title="Optimizer trial id">{t}</span>
                    })}
                    {selected.then(|| view! { <span class="pill pill-ok">"selected"</span> })}
                </div>
                <div class="variant-card-score">
                    <span class="variant-card-score-value">{format!("{:.1}%", score * 100.0)}</span>
                    {ci_label.map(|s| view! { <span class="muted text-xs">{s}</span> })}
                    <span class="pill pill-neutral text-xs">{split}</span>
                </div>
            </div>
            <div class="variant-card-meta muted text-xs">
                {format!(
                    "topK {top_k} · min {min_score:.2} · {chunk_count} chunks · avg {avg_tokens} tok · ctx {avg_retrieved} tok"
                )}
            </div>
            <div class="variant-card-bars">
                <MetricBar
                    label="Recall"
                    short="R"
                    help=RECALL_DEF.help.to_string()
                    value=m.recall_mean
                    stddev=ci_half(m.recall_ci_low, m.recall_ci_high)
                    best=bests.recall
                    kind=kind
                />
                <MetricBar
                    label="Precision"
                    short="P"
                    help=PRECISION_DEF.help.to_string()
                    value=m.precision_mean
                    stddev=ci_half(m.precision_ci_low, m.precision_ci_high)
                    best=bests.precision
                    kind=kind
                />
                <MetricBar
                    label="IoU"
                    short="IoU"
                    help=IOU_DEF.help.to_string()
                    value=m.iou_mean
                    stddev=ci_half(m.iou_ci_low, m.iou_ci_high)
                    best=bests.iou
                    kind=kind
                />
                <MetricBar
                    label="Precision-ω"
                    short="Pω"
                    help=PRECISION_OMEGA_DEF.help.to_string()
                    value=m.precision_omega_mean
                    stddev=ci_half(m.precision_omega_ci_low, m.precision_omega_ci_high)
                    best=bests.precision_omega
                    kind=kind
                />
            </div>
            <div class="variant-card-foot" on:click=|ev| ev.stop_propagation()>
                <VariantSaveButton
                    variant_label=variant_label
                    is_leader=matches!(badge, VariantBadge::Leader)
                    promote=promote
                    compact=true
                />
            </div>
        </div>
    }
}

fn download_csv(variants: &[EvaluationVariantResult]) {
    let csv = build_csv(variants);
    trigger_download(&csv, "variants.csv");
}

fn build_csv(variants: &[EvaluationVariantResult]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    out.push_str("variant,split,top_k,min_score,chunks,avg_chunk_tokens,avg_retrieved_tokens,score,score_ci_low,score_ci_high,recall,recall_ci_low,recall_ci_high,precision,precision_ci_low,precision_ci_high,iou,iou_ci_low,iou_ci_high,precision_omega,precision_omega_ci_low,precision_omega_ci_high,judge_score,selected\n");
    for v in variants {
        let m = &v.metrics;
        let score = evaluation_score(m);
        let judge = m.judge_score.map(|j| format!("{j:.4}")).unwrap_or_default();
        let label = csv_escape(&v.variant.label);
        let split = v.split.as_str();
        _ = writeln!(
            &mut out,
            "{label},{split},{},{:.3},{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{judge},{}",
            v.options.top_k,
            v.options.min_score(),
            m.chunk_count,
            m.average_chunk_tokens,
            m.average_retrieved_tokens,
            score,
            m.composite_ci_low,
            m.composite_ci_high,
            m.recall_mean,
            m.recall_ci_low,
            m.recall_ci_high,
            m.precision_mean,
            m.precision_ci_low,
            m.precision_ci_high,
            m.iou_mean,
            m.iou_ci_low,
            m.iou_ci_high,
            m.precision_omega_mean,
            m.precision_omega_ci_low,
            m.precision_omega_ci_high,
            v.selected,
        );
    }
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(target_arch = "wasm32")]
fn trigger_download(content: &str, filename: &str) {
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let array = js_sys::Array::new();
    array.push(&wasm_bindgen::JsValue::from_str(content));
    let opts = BlobPropertyBag::new();
    opts.set_type("text/csv;charset=utf-8");
    let Ok(blob) = Blob::new_with_str_sequence_and_options(&array, &opts) else {
        return;
    };
    let Ok(url) = Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        _ = Url::revoke_object_url(&url);
        return;
    };
    let Ok(elem) = document.create_element("a") else {
        _ = Url::revoke_object_url(&url);
        return;
    };
    let Ok(anchor) = elem.dyn_into::<HtmlAnchorElement>() else {
        _ = Url::revoke_object_url(&url);
        return;
    };
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    _ = Url::revoke_object_url(&url);
}

#[cfg(not(target_arch = "wasm32"))]
fn trigger_download(_content: &str, _filename: &str) {}
