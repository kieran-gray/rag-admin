use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{IndexProfileDto, IndexingDto};
use crate::ui::components::primitives::{Status, StatusPill};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Seg {
    Indexed,
    Indexing,
    Failed,
    Absent,
}

fn seg_for(profile_id: Uuid, indexings: &[IndexingDto]) -> Seg {
    match indexings
        .iter()
        .find(|i| !i.removed && i.index_profile_id == profile_id)
    {
        None => Seg::Absent,
        Some(i) if i.status.contains("Failed") => Seg::Failed,
        Some(i) if i.status.contains("Indexed") => Seg::Indexed,
        Some(_) => Seg::Indexing,
    }
}

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn CoverageCell(
    has_document: bool,
    indexings: Vec<IndexingDto>,
    profiles: Vec<IndexProfileDto>,
) -> impl IntoView {
    if !has_document {
        return view! { <StatusPill label="Not ingested".to_string() kind=Status::Stale /> }
            .into_any();
    }

    let total = profiles.len();
    if total == 0 {
        return view! { <span class="faint text-xs">"—"</span> }.into_any();
    }

    if let [only] = profiles.as_slice() {
        let (label, kind) = match seg_for(only.index_profile_id, &indexings) {
            Seg::Indexed => ("Indexed", Status::Ok),
            Seg::Indexing => ("Indexing", Status::Pending),
            Seg::Failed => ("Failed", Status::Fail),
            Seg::Absent => ("Not indexed", Status::Stale),
        };
        return view! { <StatusPill label=label.to_string() kind=kind /> }.into_any();
    }

    let segs: Vec<(Seg, String)> = profiles
        .iter()
        .map(|p| (seg_for(p.index_profile_id, &indexings), p.name.clone()))
        .collect();
    let indexed = segs.iter().filter(|(s, _)| *s == Seg::Indexed).count();

    view! {
        <div class="coverage-cell" title=format!("{indexed} of {total} profiles indexed")>
            <div class="coverage-track">
                {segs.into_iter().map(|(s, name)| {
                    let (cls, state) = match s {
                        Seg::Indexed => ("coverage-seg coverage-seg-ok", "indexed"),
                        Seg::Indexing => ("coverage-seg coverage-seg-pending", "indexing"),
                        Seg::Failed => ("coverage-seg coverage-seg-fail", "failed"),
                        Seg::Absent => ("coverage-seg", "not indexed"),
                    };
                    view! { <span class=cls title=format!("{name}: {state}")></span> }
                }).collect_view()}
            </div>
            <span class="coverage-count">{format!("{indexed}/{total}")}</span>
        </div>
    }
    .into_any()
}
