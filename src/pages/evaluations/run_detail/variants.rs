use leptos::prelude::*;

use crate::components::primitives::{MetricBar, MetricKind, Surface};
use crate::shared::{evaluation_score, EvaluationVariantResult};

use super::promote::{PromoteHandle, VariantSaveButton};
use super::shared::{ci_half, metric_cell, row_key, variant_display, MetricBests, METRIC_DEFS};
use super::summary::AxisLegend;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantsView {
    Bars,
    Table,
}

#[component]
pub(super) fn VariantsSection(
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

    let body = move || {
        match mode.get() {
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
    }
    };

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

    view! {
        <Surface title="Variants".to_string() actions=actions sticky_header=true>
            {status_banner}
            {body}
        </Surface>
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

fn cmp_variants(
    a: &EvaluationVariantResult,
    b: &EvaluationVariantResult,
    key: SortKey,
) -> std::cmp::Ordering {
    use std::cmp::Ordering::Equal;
    match key {
        SortKey::Variant => a.variant.label.cmp(&b.variant.label),
        SortKey::Split => a.split.as_str().cmp(b.split.as_str()),
        SortKey::TopK => a.options.top_k.cmp(&b.options.top_k),
        SortKey::MinScore => a.options.min_score_milli.cmp(&b.options.min_score_milli),
        SortKey::Chunks => a.metrics.chunk_count.cmp(&b.metrics.chunk_count),
        SortKey::AvgTok => a
            .metrics
            .average_chunk_tokens
            .cmp(&b.metrics.average_chunk_tokens),
        SortKey::Recall => a
            .metrics
            .recall_mean
            .partial_cmp(&b.metrics.recall_mean)
            .unwrap_or(Equal),
        SortKey::Precision => a
            .metrics
            .precision_mean
            .partial_cmp(&b.metrics.precision_mean)
            .unwrap_or(Equal),
        SortKey::Iou => a
            .metrics
            .iou_mean
            .partial_cmp(&b.metrics.iou_mean)
            .unwrap_or(Equal),
        SortKey::Pomega => a
            .metrics
            .precision_omega_mean
            .partial_cmp(&b.metrics.precision_omega_mean)
            .unwrap_or(Equal),
        SortKey::Score => evaluation_score(&a.metrics)
            .partial_cmp(&evaluation_score(&b.metrics))
            .unwrap_or(Equal),
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
    let class = if numeric { "num sortable" } else { "sortable" };
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
