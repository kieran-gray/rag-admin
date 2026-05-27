use leptos::prelude::*;

use crate::shared::EvaluationVariantResult;
use crate::ui::components::primitives::Surface;

pub(crate) struct CategoryRow {
    pub category: String,
    pub n: usize,
    pub recall: f32,
    pub precision: f32,
    pub iou: f32,
}

pub(crate) fn category_breakdown(leader: &EvaluationVariantResult) -> Vec<CategoryRow> {
    use std::collections::BTreeMap;
    if leader.question_results.is_empty() {
        return Vec::new();
    }
    let mut by_cat: BTreeMap<String, (usize, f32, f32, f32)> = BTreeMap::new();
    for q in &leader.question_results {
        let key = format!("{} · {}", q.operation, q.evidence_kind);
        let entry = by_cat.entry(key).or_insert((0, 0.0, 0.0, 0.0));
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

#[component]
pub(crate) fn CategoryBreakdownPanel(rows: Vec<CategoryRow>) -> impl IntoView {
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
                        let cell = move |value: f32| -> leptos::prelude::AnyView {
                            view! {
                                <td class="num">{format!("{:.1}%", value * 100.0)}</td>
                            }.into_any()
                        };
                        view! {
                            <tr>
                                <td>{r.category}</td>
                                <td class="num">{r.n}</td>
                                {cell(r.recall)}
                                {cell(r.precision)}
                                {cell(r.iou)}
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </Surface>
    }
}
