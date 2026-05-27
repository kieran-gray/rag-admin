use leptos::prelude::*;

use crate::shared::contracts::Timings;

#[component]
pub fn LatencyBadge(timings: Timings) -> impl IntoView {
    let segments = build_segments(&timings);
    let total = format!("{}ms", timings.total_ms);
    view! {
        <span class="latency-badge" title="Per-stage latency">
            <span class="latency-badge-total">{total}</span>
            {segments.into_iter().map(|s| view! {
                <span class="latency-badge-segment">
                    <span class="latency-badge-segment-label">{s.label}</span>
                    <span class="latency-badge-segment-value">{format!("{}ms", s.value)}</span>
                </span>
            }).collect_view()}
        </span>
    }
}

struct Segment {
    label: &'static str,
    value: u32,
}

fn build_segments(t: &Timings) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    if t.embed_ms > 0 {
        out.push(Segment {
            label: "embed",
            value: t.embed_ms,
        });
    }
    if t.retrieve_ms > 0 {
        out.push(Segment {
            label: "retrieve",
            value: t.retrieve_ms,
        });
    }
    if let Some(v) = t.rerank_ms {
        out.push(Segment {
            label: "rerank",
            value: v,
        });
    }
    if let Some(v) = t.generate_ms {
        out.push(Segment {
            label: "gen",
            value: v,
        });
    }
    out
}
