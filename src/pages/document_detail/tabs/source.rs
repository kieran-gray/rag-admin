use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::components::primitives::{EmptyState, Surface};
use crate::server_functions::source_document::get_document_source;
use crate::shared::{MarkdownBlockDto, MarkdownBlockKindDto, SourceDocumentMarkdownDto};

#[component]
pub fn SourceTab(source_ref: String) -> impl IntoView {
    let source_ref_stored = StoredValue::new(source_ref.clone());
    let body = Resource::new(
        move || source_ref_stored.get_value(),
        move |slug| async move { get_document_source(slug).await.map_err(|e| e.to_string()) },
    );

    let (show_raw, set_show_raw) = signal(false);

    let query = use_query_map();
    let ref_range = Memo::new(move |_| {
        query.with(|q| {
            let start = q.get("ref_start").and_then(|v| v.parse::<u32>().ok());
            let end = q.get("ref_end").and_then(|v| v.parse::<u32>().ok());
            match (start, end) {
                (Some(s), Some(e)) if e > s => Some((s, e)),
                _ => None,
            }
        })
    });

    view! {
        <Transition fallback=|| view! {
            <Surface><p class="muted">"Loading source…"</p></Surface>
        }>
            {move || body.get().map(|res| match res {
                Err(e) => view! {
                    <Surface>
                        <div class="log-line-error">{format!("Failed to load source: {e}")}</div>
                    </Surface>
                }.into_any(),
                Ok(None) => view! {
                    <Surface>
                        <EmptyState
                            title="No source on file"
                            body="This document hasn't been imported yet, so there is no markdown body to render.".to_string()
                        />
                    </Surface>
                }.into_any(),
                Ok(Some(doc)) => view! {
                    <SourceBody doc=doc show_raw=show_raw set_show_raw=set_show_raw ref_range=ref_range />
                }.into_any(),
            })}
        </Transition>
    }
}

#[component]
fn SourceBody(
    doc: SourceDocumentMarkdownDto,
    show_raw: ReadSignal<bool>,
    set_show_raw: WriteSignal<bool>,
    ref_range: Memo<Option<(u32, u32)>>,
) -> impl IntoView {
    let doc_stored = StoredValue::new(doc);

    let toggle = move |_| set_show_raw.update(|v| *v = !*v);

    let body_view = move || {
        let doc = doc_stored.get_value();
        if show_raw.get() {
            view! { <RawMarkdown source=doc.source /> }.into_any()
        } else {
            view! { <RenderedMarkdown blocks=doc.blocks ref_range=ref_range /> }.into_any()
        }
    };

    let actions: Box<dyn Fn() -> leptos::prelude::AnyView + Send + Sync> = Box::new(move || {
        view! {
            <button
                type="button"
                class="btn"
                on:click=toggle
            >
                {move || if show_raw.get() { "Rendered" } else { "View raw markdown" }}
            </button>
        }
        .into_any()
    });

    let title = doc_stored.with_value(|d| format!("Source · v{}", d.version));

    view! {
        <Surface title=title actions=actions>
            {body_view}
        </Surface>
    }
}

#[component]
fn RawMarkdown(source: String) -> impl IntoView {
    view! {
        <pre class="md-raw">{source}</pre>
    }
}

#[component]
fn RenderedMarkdown(
    blocks: Vec<MarkdownBlockDto>,
    ref_range: Memo<Option<(u32, u32)>>,
) -> impl IntoView {
    let blocks = StoredValue::new(blocks);

    Effect::new(move |_| {
        if ref_range.get().is_some() {
            request_scroll_to_highlight();
        }
    });

    let rows = move || {
        let range = ref_range.get();
        let mut anchor_assigned = false;
        blocks
            .with_value(|bs| bs.clone())
            .into_iter()
            .map(|block| {
                let highlighted =
                    range.is_some_and(|(s, e)| block.char_start < e && block.char_end > s);
                let is_anchor = highlighted && !anchor_assigned;
                if is_anchor {
                    anchor_assigned = true;
                }
                let mut class = format!("md-block md-{}", kind_class(block.kind));
                if highlighted {
                    class.push_str(" is-highlighted");
                }
                let id_attr = if is_anchor { Some("ref-anchor") } else { None };

                view! {
                    <div
                        class=class
                        id=id_attr
                        data-start=block.char_start.to_string()
                        data-end=block.char_end.to_string()
                        inner_html=block.html
                    ></div>
                }
            })
            .collect_view()
    };

    view! {
        <div class="md-document">
            {rows}
        </div>
    }
}

fn kind_class(kind: MarkdownBlockKindDto) -> &'static str {
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

#[cfg(feature = "hydrate")]
fn request_scroll_to_highlight() {
    use wasm_bindgen::JsCast;
    leptos::task::spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(50).await;
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(el) = document.get_element_by_id("ref-anchor") {
                    if let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() {
                        el.scroll_into_view_with_bool(true);
                    }
                }
            }
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn request_scroll_to_highlight() {}
