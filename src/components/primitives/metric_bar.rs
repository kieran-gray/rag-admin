use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetricKind {
    #[default]
    Default,
    Best,
}

#[component]
pub fn MetricBar(
    #[prop(into)] label: String,
    #[prop(optional, into)] short: Option<String>,
    #[prop(optional, into)] help: Option<String>,
    value: f32,
    #[prop(optional)] stddev: Option<f32>,
    #[prop(optional)] best: Option<f32>,
    #[prop(optional)] kind: Option<MetricKind>,
) -> impl IntoView {
    let kind = kind.unwrap_or_default();
    let value = value.clamp(0.0, 1.0);

    let bar_colour = match kind {
        MetricKind::Best => "var(--color-accent)",
        MetricKind::Default => "var(--status-info)",
    };

    let value_pct = format!("{:.1}%", value * 100.0);

    let value_label = match stddev {
        Some(s) if s > 0.0005 => format!("{:.1}% ± {:.1}", value * 100.0, s * 100.0),
        _ => format!("{:.1}%", value * 100.0),
    };

    let stddev_band = stddev.and_then(|s| {
        if s <= 0.0005 {
            return None;
        }
        let s = s.clamp(0.0, 1.0);
        let left = ((value - s).max(0.0) * 100.0).clamp(0.0, 100.0);
        let right = ((value + s).min(1.0) * 100.0).clamp(0.0, 100.0);
        let width = (right - left).max(0.0);
        Some(view! {
            <span
                class="metric-bar-stddev"
                style=format!("left: {left:.2}%; width: {width:.2}%")
                aria-hidden="true"
            ></span>
        })
    });

    let best_marker = best.and_then(|b| {
        let b = b.clamp(0.0, 1.0);
        if (b - value).abs() < 0.001 {
            return None;
        }
        let left = (b * 100.0).clamp(0.0, 100.0);
        Some(view! {
            <span
                class="metric-bar-best-tick"
                style=format!("left: {left:.2}%")
                title=format!("Best in run: {:.1}%", b * 100.0)
                aria-hidden="true"
            ></span>
        })
    });

    view! {
        <div class="metric-bar-row" title=help.clone().unwrap_or_default()>
            <div class="metric-bar-label">
                <span class="metric-bar-label-name">{label}</span>
                {short.map(|s| view! {
                    <span class="metric-bar-label-short">{s}</span>
                })}
            </div>
            <div class="metric-bar-track" role="img" aria-label=value_pct>

                <span class="metric-bar-tick" style="left: 25%"></span>
                <span class="metric-bar-tick" style="left: 50%"></span>
                <span class="metric-bar-tick" style="left: 75%"></span>
                {stddev_band}
                <span
                    class="metric-bar-fill"
                    style=format!("width: {}; background-color: {}", value_pct, bar_colour)
                ></span>
                {best_marker}
            </div>
            <span class="metric-bar-value">{value_label}</span>
        </div>
    }
}
