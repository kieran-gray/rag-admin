use leptos::prelude::*;

use crate::shared::contracts::QueryHit;
use crate::ui::components::playground::HitCard;
use crate::ui::pages::shared::{hit_display_title, hit_source_link};

#[allow(clippy::needless_pass_by_value)]
#[component]
pub(super) fn SourcesList(question: String, hits: Vec<QueryHit>) -> impl IntoView {
    view! {
        <div class="chat-sources">
            {hits.into_iter().enumerate().map(|(i, hit)| {
                let title = hit_display_title(&hit);
                let heading = hit.heading.clone();
                let snippet = hit.snippet.clone();
                let source = hit_source_link(&hit);
                let embed_link = format!(
                    "/playground/embed?a={}&b={}",
                    urlencoding::encode(&question),
                    urlencoding::encode(&snippet),
                );
                let footer_actions: Children = Box::new(move || view! {
                    {source.clone().map(|href| view! {
                        <a class="btn btn-ghost btn-sm" href=href>"Open document"</a>
                    })}
                    <a
                        class="btn btn-ghost btn-sm"
                        href=embed_link
                        title="Compare this snippet against the question on the Embed page"
                    >
                        "Open in Embed"
                    </a>
                }.into_any());
                view! {
                    <HitCard
                        rank=i + 1
                        score=hit.score
                        title=title
                        heading=heading
                        snippet=snippet
                        query=None
                        badge=None
                        header_actions=None
                        footer_actions=Some(footer_actions)
                        details=None
                        highlighted=false
                        extra_class=Some("chat-source".to_string())
                    />
                }
            }).collect_view()}
        </div>
    }
}
