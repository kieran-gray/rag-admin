use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::QueryResult;
use crate::ui::components::playground::marks_store::{Mark, MarkKind};

#[cfg(feature = "hydrate")]
const SUBMIT_QUERY_FOCUS_SELECTOR: &str = ".query-input-textarea";

pub(super) fn cycle_highlighted_hit(
    highlighted_hit: RwSignal<Option<usize>>,
    result: RwSignal<Option<QueryResult>>,
    delta: i32,
) {
    let count = result.with(|r| r.as_ref().map_or(0, |x| x.hits.len()));
    if count == 0 {
        return;
    }
    highlighted_hit.update(|h| {
        let next = match *h {
            None => {
                if delta > 0 {
                    0
                } else {
                    count - 1
                }
            }
            Some(i) => {
                let i = i as i32 + delta;
                let n = count as i32;
                ((i % n + n) % n) as usize
            }
        };
        *h = Some(next);
    });
}

pub(super) fn highlighted_hit_chunk(
    highlighted_hit: RwSignal<Option<usize>>,
    result: RwSignal<Option<QueryResult>>,
) -> Option<(String, Uuid)> {
    let idx = highlighted_hit.get_untracked()?;
    result.with_untracked(|r| {
        let res = r.as_ref()?;
        let hit = res.hits.get(idx)?;
        Some((res.query.clone(), hit.chunk_id?))
    })
}

pub(super) fn set_mark_to(
    marks: RwSignal<Vec<Mark>>,
    query: String,
    chunk_id: Uuid,
    kind: MarkKind,
) {
    marks.update(|m| {
        let position = m
            .iter()
            .position(|e| e.query == query && e.chunk_id == chunk_id);
        match position {
            Some(idx) => {
                let same_kind = m.get(idx).is_some_and(|e| e.kind == kind);
                if same_kind {
                    m.remove(idx);
                } else if let Some(entry) = m.get_mut(idx) {
                    entry.kind = kind;
                }
            }
            None => m.push(Mark {
                query,
                chunk_id,
                kind,
            }),
        }
    });
}

pub(super) fn is_global_target(ev: &KeyboardEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    let Some(target) = ev.target() else {
        return true;
    };
    let Ok(el) = target.dyn_into::<web_sys::Element>() else {
        return true;
    };
    let tag = el.tag_name().to_lowercase();
    !matches!(tag.as_str(), "input" | "textarea" | "select")
}

#[cfg(feature = "hydrate")]
pub(super) fn focus_query_textarea() {
    use leptos::wasm_bindgen::JsCast;
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(Some(el)) = document.query_selector(SUBMIT_QUERY_FOCUS_SELECTOR) else {
        return;
    };
    if let Ok(html) = el.dyn_into::<web_sys::HtmlElement>() {
        drop(html.focus());
    }
}

#[cfg(not(feature = "hydrate"))]
pub(super) fn focus_query_textarea() {}
