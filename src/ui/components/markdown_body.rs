use leptos::prelude::*;

use crate::shared::contracts::{MarkdownBlockDto, MarkdownBlockKindDto};

#[component]
pub fn MarkdownBody(blocks: Vec<MarkdownBlockDto>) -> impl IntoView {
    view! {
        <div class="md-document">
            {blocks.into_iter().map(|block| view! {
                <div
                    class=format!("md-block md-{}", block_kind_class(block.kind))
                    inner_html=block.html
                ></div>
            }).collect_view()}
        </div>
    }
}

pub fn block_kind_class(kind: MarkdownBlockKindDto) -> &'static str {
    match kind {
        MarkdownBlockKindDto::Heading => "heading",
        MarkdownBlockKindDto::Paragraph => "paragraph",
        MarkdownBlockKindDto::List => "list",
        MarkdownBlockKindDto::CodeFence => "code",
        MarkdownBlockKindDto::BlockQuote => "blockquote",
        MarkdownBlockKindDto::Table => "table",
        MarkdownBlockKindDto::Html => "html",
        MarkdownBlockKindDto::ThematicBreak => "rule",
        MarkdownBlockKindDto::Other => "other",
    }
}
