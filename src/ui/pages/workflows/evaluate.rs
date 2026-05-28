use leptos::either::Either;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use crate::server_functions::source_document::list_documents;
use crate::shared::contracts::{aggregate_type, DocumentListItemDto};
use crate::ui::components::primitives::{
    EmptyState, PageHeader, SkeletonColumn, SkeletonRows, Surface, TitleCell,
};
use crate::ui::pages::shared::{document_type_label, format_when};
use crate::ui::state::event_bus::use_invalidator;

#[component]
pub fn EvaluateWorkflowPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });
    let docs = Resource::new(
        move || invalidator.get(),
        |_| async move { list_documents().await.unwrap_or_default() },
    );

    view! {
        <div>
            <PageHeader
                title="Pick a document"
                subtitle="Build a map, generate questions, run evaluations.".to_string()
            />

            <Surface flush=true>
                <Transition fallback=move || view! { <SkeletonEvaluateTable /> }>
                    {move || docs.get().map(|list| {
                        let registered: Vec<DocumentListItemDto> = list.into_iter()
                            .filter(|d| d.document_id.is_some())
                            .collect();
                        if registered.is_empty() {
                            Either::Left(view! {
                                <div class="p-6">
                                    <EmptyState
                                        title="No imported documents"
                                        body="Import a document from the Documents page first, then evaluate it here.".to_string()
                                    />
                                </div>
                            })
                        } else {
                            Either::Right(view! { <EvaluateTable docs=registered /> })
                        }
                    })}
                </Transition>
            </Surface>
        </div>
    }
}

#[component]
fn EvaluateTableHead() -> impl IntoView {
    view! {
        <thead>
            <tr>
                <th class="w-[56%]">"Document"</th>
                <th class="w-[20%]">"Source"</th>
                <th class="w-[20%] text-right">"When"</th>
                <th class="w-[4%] text-right"></th>
            </tr>
        </thead>
    }
}

#[component]
fn EvaluateTable(docs: Vec<DocumentListItemDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <EvaluateTableHead />
            <tbody>
                {docs.into_iter().map(|d| view! { <EvaluateRow doc=d /> }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn SkeletonEvaluateTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <EvaluateTableHead />
            <tbody>
                <SkeletonRows columns=vec![
                    SkeletonColumn::with_sub("70%", "40%"),
                    SkeletonColumn::new("50%"),
                    SkeletonColumn::right("4rem"),
                    SkeletonColumn::empty(),
                ] />
            </tbody>
        </table>
    }
}

#[component]
fn EvaluateRow(doc: DocumentListItemDto) -> impl IntoView {
    let Some(document_id) = doc.document_id else {
        return view! { <tr></tr> }.into_any();
    };
    let type_label = document_type_label(&doc.document_type);
    let source_label = doc.source.label();
    let when = format_when(&doc.updated_at);
    let when_full = doc.updated_at.clone();
    let title = doc.title.clone();
    let sub = format!("{type_label} · {}", doc.source_ref_key);

    let href = format!("/evaluate/by-id/{document_id}");
    let nav_href = href.clone();
    let on_row_click = move |ev: leptos::ev::MouseEvent| {
        if ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.shift_key() {
            return;
        }
        use_navigate()(&nav_href, NavigateOptions::default());
    };

    view! {
        <tr on:click=on_row_click>
            <td>
                <A href=href.clone() attr:class="block">
                    <TitleCell title=title sub=sub />
                </A>
            </td>
            <td>
                <span class="text-xs muted truncate">{source_label}</span>
            </td>
            <td class="text-right text-xs muted whitespace-nowrap" title=when_full>{when}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
    .into_any()
}
