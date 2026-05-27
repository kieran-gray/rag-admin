use leptos::prelude::*;

use crate::ui::components::playground::HighlightedSnippet;

#[derive(Clone)]
pub struct HitBadge {
    pub label: String,
    pub class: String,
}

#[component]
pub fn HitCard(
    rank: usize,
    score: f32,
    #[prop(into)] title: String,
    heading: Option<String>,
    #[prop(into)] snippet: String,
    query: Option<String>,
    badge: Option<HitBadge>,
    header_actions: Option<Children>,
    footer_actions: Option<Children>,
    details: Option<Children>,
    highlighted: bool,
    extra_class: Option<String>,
) -> impl IntoView {
    let extra = extra_class.unwrap_or_default();
    let mut class = format!("playground-hit {extra}");
    if highlighted {
        class.push_str(" playground-hit-highlighted");
    }
    let score_display = format!("{score:.3}");
    let query_for_snippet = query.unwrap_or_default();

    view! {
        <article class=class>
            <header class="playground-hit-head">
                <span class="playground-hit-rank">{rank}</span>
                <span class="playground-hit-score">{score_display}</span>
                <div class="playground-hit-title">
                    <div class="playground-hit-doc">{title}</div>
                    {heading.map(|h| view! { <div class="playground-hit-heading muted">{h}</div> })}
                </div>
                {badge.map(|b| view! {
                    <span class=b.class>{b.label}</span>
                })}
                {header_actions.map(|c| view! {
                    <div class="playground-hit-actions">{c()}</div>
                })}
            </header>
            <HighlightedSnippet text=snippet query=query_for_snippet />
            {footer_actions.map(|c| view! {
                <div class="playground-hit-footer">{c()}</div>
            })}
            {details.map(|c| view! {
                <details class="playground-hit-details-wrap">
                    <summary class="playground-hit-reveal">"Details"</summary>
                    {c()}
                </details>
            })}
        </article>
    }
}
