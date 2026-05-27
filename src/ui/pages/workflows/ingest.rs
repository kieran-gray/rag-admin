use leptos::either::Either;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use crate::server_functions::source_document::list_documents;
use crate::shared::contracts::{aggregate_type, DocumentListItemDto};
use crate::ui::components::primitives::{
    EmptyState, PageHeader, SkeletonColumn, SkeletonRows, StatusPill, Surface, TitleCell,
};
use crate::ui::pages::shared::{document_type_label, format_when, indexing_summary};
use crate::ui::state::event_bus::use_invalidator;

#[component]
pub fn IngestWorkflowPage() -> impl IntoView {
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
                eyebrow="Workflows / Ingest".to_string()
                subtitle="Chunk, embed, and index a document for retrieval.".to_string()
            />

            <Surface flush=true>
                <Transition fallback=move || view! { <SkeletonIngestTable /> }>
                    {move || docs.get().map(|list| {
                        let registered: Vec<DocumentListItemDto> = list.into_iter()
                            .filter(|d| d.document_id.is_some())
                            .collect();
                        if registered.is_empty() {
                            Either::Left(view! {
                                <div class="p-6">
                                    <EmptyState
                                        title="No imported documents"
                                        body="Import a document from the Documents page first, then start ingesting it here.".to_string()
                                    />
                                </div>
                            })
                        } else {
                            Either::Right(view! { <IngestTable docs=registered /> })
                        }
                    })}
                </Transition>
            </Surface>
        </div>
    }
}

#[component]
fn IngestTableHead() -> impl IntoView {
    view! {
        <thead>
            <tr>
                <th class="w-[44%]">"Document"</th>
                <th class="w-[20%]">"State"</th>
                <th class="w-[16%]">"Source"</th>
                <th class="w-[16%] text-right">"When"</th>
                <th class="w-[4%] text-right"></th>
            </tr>
        </thead>
    }
}

#[component]
fn IngestTable(docs: Vec<DocumentListItemDto>) -> impl IntoView {
    view! {
        <table class="data-table">
            <IngestTableHead />
            <tbody>
                {docs.into_iter().map(|d| view! { <IngestRow doc=d /> }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn SkeletonIngestTable() -> impl IntoView {
    view! {
        <table class="data-table">
            <IngestTableHead />
            <tbody>
                <SkeletonRows columns=vec![
                    SkeletonColumn::with_sub("70%", "40%"),
                    SkeletonColumn::new("60%"),
                    SkeletonColumn::new("50%"),
                    SkeletonColumn::right("4rem"),
                    SkeletonColumn::empty(),
                ] />
            </tbody>
        </table>
    }
}

#[component]
fn IngestRow(doc: DocumentListItemDto) -> impl IntoView {
    let summary = indexing_summary(&doc.indexings, doc.latest_version);
    let type_label = document_type_label(&doc.document_type);
    let source_label = doc.source.label();
    let when = format_when(&doc.updated_at);
    let when_full = doc.updated_at.clone();
    let title = doc.title.clone();
    let sub = format!("{type_label} · {}", doc.source_ref_key);

    let href = format!(
        "/documents/{}/{}/ingest",
        doc.document_type.to_lowercase(),
        urlencoding::encode(&doc.source_ref_key),
    );
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
                <StatusPill label=summary.label kind=summary.status />
            </td>
            <td>
                <span class="text-xs muted truncate">{source_label}</span>
            </td>
            <td class="text-right text-xs muted whitespace-nowrap" title=when_full>{when}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}
