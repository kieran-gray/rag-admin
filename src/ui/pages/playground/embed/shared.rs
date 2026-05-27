use leptos::prelude::*;

use crate::shared::contracts::EmbeddingModelDto;
use crate::shared::EmbedInputType;
use crate::ui::components::primitives::MetricKind;

pub(super) const TOKEN_WARN_CHARS: usize = 30_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EmbedMode {
    Pairwise,
    Ranked,
    Matrix,
}

impl EmbedMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Pairwise => "Pairwise",
            Self::Ranked => "Ranked",
            Self::Matrix => "Matrix",
        }
    }
}

pub(super) struct SimilarityBucket {
    pub label: &'static str,
    pub kind: MetricKind,
}

pub(super) fn similarity_bucket(s: f32) -> SimilarityBucket {
    if s >= 0.85 {
        SimilarityBucket {
            label: "Highly similar",
            kind: MetricKind::Best,
        }
    } else if s >= 0.65 {
        SimilarityBucket {
            label: "Related",
            kind: MetricKind::Default,
        }
    } else if s >= 0.45 {
        SimilarityBucket {
            label: "Loosely related",
            kind: MetricKind::Default,
        }
    } else {
        SimilarityBucket {
            label: "Unrelated",
            kind: MetricKind::Default,
        }
    }
}

pub(super) fn norm_tone(n: f32) -> &'static str {
    if norm_warn(n) {
        "log-line-warn"
    } else {
        ""
    }
}

pub(super) fn norm_warn(n: f32) -> bool {
    (n - 1.0).abs() >= 0.1
}

pub(super) fn preview(text: &str, max: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(max).collect();
    out.push('…');
    out
}

#[component]
pub(super) fn ModeToggle(current: RwSignal<EmbedMode>, disabled: Signal<bool>) -> impl IntoView {
    let modes = [EmbedMode::Pairwise, EmbedMode::Ranked, EmbedMode::Matrix];
    view! {
        <div class="embed-mode-toggle" role="tablist">
            {modes.into_iter().map(|m| {
                let label = m.label();
                let is_current = Memo::new(move |_| current.get() == m);
                view! {
                    <button
                        type="button"
                        role="tab"
                        class=move || if is_current.get() { "embed-mode-btn embed-mode-btn-active" } else { "embed-mode-btn" }
                        disabled=disabled
                        on:click=move |_| current.set(m)
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
pub(super) fn InputTypeSelect(
    value: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
) -> impl IntoView {
    let options = [
        EmbedInputType::Plain,
        EmbedInputType::Query,
        EmbedInputType::Passage,
    ];
    view! {
        <select
            class="embed-input-type"
            disabled=disabled
            on:change=move |ev| {
                let v = event_target_value(&ev);
                let kind = match v.as_str() {
                    "query" => EmbedInputType::Query,
                    "passage" => EmbedInputType::Passage,
                    _ => EmbedInputType::Plain,
                };
                value.set(kind);
            }
        >
            {options.into_iter().map(|opt| {
                let label = opt.label();
                let opt_str = opt.as_str();
                view! {
                    <option value=opt_str selected=move || value.get() == opt>{label}</option>
                }
            }).collect_view()}
        </select>
    }
}

#[component]
pub(super) fn CharCount(text: RwSignal<String>) -> impl IntoView {
    let count = Memo::new(move |_| text.with(|t| t.chars().count()));
    let warn = Memo::new(move |_| count.get() > TOKEN_WARN_CHARS);
    let class = move || {
        if warn.get() {
            "embed-char-count embed-char-count-warn"
        } else {
            "embed-char-count"
        }
    };
    view! {
        <span class=class title="char count · approx tokens">
            {move || {
                let c = count.get();
                let tokens = c.div_ceil(4);
                format!("{c} chars · ≈{tokens} tok")
            }}
        </span>
    }
}

#[component]
pub(super) fn ThresholdLegend() -> impl IntoView {
    view! {
        <div class="embed-threshold-legend">
            <span class="embed-threshold-legend-stop" style="left: 45%">"0.45"</span>
            <span class="embed-threshold-legend-stop" style="left: 65%">"0.65"</span>
            <span class="embed-threshold-legend-stop" style="left: 85%">"0.85"</span>
            <span class="embed-threshold-legend-track" aria-hidden="true"></span>
            <div class="embed-threshold-legend-labels">
                <span class="faint">"unrelated"</span>
                <span class="faint">"loose"</span>
                <span class="faint">"related"</span>
                <span class="faint">"highly"</span>
            </div>
        </div>
    }
}

#[component]
pub(super) fn Stat(
    label: String,
    value: String,
    #[prop(default = "")] tone: &'static str,
) -> impl IntoView {
    let value_class = if tone.is_empty() {
        "embed-stat-value".to_string()
    } else {
        format!("embed-stat-value {tone}")
    };
    view! {
        <div class="embed-stat">
            <div class="eyebrow">{label}</div>
            <div class=value_class>{value}</div>
        </div>
    }
}

#[component]
pub(super) fn EmbeddingModelPicker(
    models: Vec<EmbeddingModelDto>,
    value: RwSignal<String>,
    disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <label class="playground-picker-label">
            <span class="eyebrow">"Model"</span>
            <select
                class="playground-profile-picker"
                disabled=disabled
                on:change=move |ev| value.set(event_target_value(&ev))
            >
                {models.into_iter().map(|m| {
                    let key = m.model.clone();
                    let value_match = key.clone();
                    let label = format!("{} · {}d · {}", m.model, m.dimensions, m.kind.display_label());
                    view! {
                        <option
                            value=key
                            selected=move || value.get() == value_match
                        >
                            {label}
                        </option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}
