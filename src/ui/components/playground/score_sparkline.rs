use leptos::prelude::*;

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn ScoreSparkline(scores: Vec<f32>, min_score: f32) -> impl IntoView {
    if scores.is_empty() {
        return ().into_any();
    }
    let width = 72.0_f32;
    let height = 24.0_f32;
    let n = scores.len() as f32;
    let bar_gap = 1.0_f32;
    let bar_width = ((width - bar_gap * (n - 1.0).max(0.0)) / n).max(1.0);

    let bars = scores
        .iter()
        .enumerate()
        .map(|(i, score)| {
            let s = score.clamp(0.0, 1.0);
            let bar_h = (s * height).max(1.0);
            let x = i as f32 * (bar_width + bar_gap);
            let y = height - bar_h;
            let above = *score >= min_score;
            let class = if above {
                "score-sparkline-bar"
            } else {
                "score-sparkline-bar score-sparkline-bar-below"
            };
            view! {
                <rect
                    class=class
                    x=format!("{x:.2}")
                    y=format!("{y:.2}")
                    width=format!("{bar_width:.2}")
                    height=format!("{bar_h:.2}")
                ></rect>
            }
        })
        .collect_view();

    let threshold_y = height - (min_score.clamp(0.0, 1.0) * height);

    view! {
        <svg
            class="score-sparkline"
            viewBox=format!("0 0 {width} {height}")
            width=width.to_string()
            height=height.to_string()
            preserveAspectRatio="none"
            role="img"
            aria-label="Score distribution"
        >
            {bars}
            <line
                class="score-sparkline-threshold"
                x1="0"
                x2=width.to_string()
                y1=format!("{threshold_y:.2}")
                y2=format!("{threshold_y:.2}")
            ></line>
        </svg>
    }
    .into_any()
}
