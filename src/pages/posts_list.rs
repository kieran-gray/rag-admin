use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::event_bus::use_invalidator;
use crate::components::primitives::{EmptyState, PageHeader, Status, StatusPill, Surface};
use crate::server_functions::source_document::list_documents_with_status;
use crate::shared::{aggregate_type, DocumentListItemDto};

#[component]
pub fn PostsListPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });
    let docs = Resource::new(
        move || invalidator.get(),
        |_| async move { list_documents_with_status().await },
    );

    view! {
        <div>
            <PageHeader
                title="Documents"
                subtitle="Source documents discovered by the registered adapters.".to_string()
                actions=Box::new(|| view! {



                    <button class="btn btn-primary" disabled=true title="Coming soon">
                        "+ Import"
                    </button>
                }.into_any())
            />

            <Suspense fallback=|| view! {
                <Surface flush=true>
                    <div class="p-6 muted text-sm">"Loading documents…"</div>
                </Surface>
            }>
                {move || docs.get().map(|res| match res {
                    Ok(list) if list.is_empty() => view! {
                        <Surface>
                            <EmptyState
                                title="No documents yet"
                                body="Import sources from the upstream blog or another adapter to begin.".to_string()
                            />
                        </Surface>
                    }.into_any(),
                    Ok(list) => view! { <DocumentsTable docs=list /> }.into_any(),
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        </Surface>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn DocumentsTable(docs: Vec<DocumentListItemDto>) -> impl IntoView {
    let total = docs.len();
    let indexed = docs
        .iter()
        .filter(|d| d.indexings.iter().any(|i| i.status.contains("Indexed")))
        .count();
    let failed = docs
        .iter()
        .filter(|d| d.indexings.iter().any(|i| i.status.contains("Failed")))
        .count();
    let in_progress = docs
        .iter()
        .filter(|d| {
            d.document_id.is_some()
                && !d.indexings.is_empty()
                && d.indexings
                    .iter()
                    .all(|i| !i.status.contains("Indexed") && !i.status.contains("Failed"))
        })
        .count();

    view! {
        <Surface flush=true>
            <table class="data-table">
                <thead>
                    <tr>
                        <th class="w-[36%]">"Title"</th>
                        <th>"Type"</th>
                        <th>"Status"</th>
                        <th class="text-right">"Version"</th>
                        <th class="w-8 text-right"></th>
                    </tr>
                </thead>
                <tbody>
                    {docs.into_iter().map(|doc| view! { <DocumentRow doc /> }).collect_view()}
                </tbody>
            </table>
            <div class="px-4 py-2.5 border-t border-[var(--color-border)] flex items-center gap-4 text-xs muted">
                <span>{format!("{total} documents")}</span>
                <span class="faint">"·"</span>
                <span>{format!("{indexed} indexed")}</span>
                {(in_progress > 0).then(|| view! {
                    <>
                        <span class="faint">"·"</span>
                        <span style="color: var(--status-pending)">
                            {format!("{in_progress} in progress")}
                        </span>
                    </>
                })}
                {(failed > 0).then(|| view! {
                    <>
                        <span class="faint">"·"</span>
                        <span style="color: var(--status-fail)">
                            {format!("{failed} failed")}
                        </span>
                    </>
                })}
            </div>
        </Surface>
    }
}

#[component]
fn DocumentRow(doc: DocumentListItemDto) -> impl IntoView {
    let href = format!(
        "/documents/{}/{}",
        doc.document_type.to_lowercase(),
        doc.source_ref_key,
    );
    let (status_label, status_kind) = ingest_status(&doc);
    let version_label = doc
        .latest_version
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "—".into());
    let type_label = document_type_label(&doc.document_type).to_string();
    let title = doc.title.clone();
    let source_ref = doc.source_ref_key.clone();

    view! {
        <tr>
            <td>
                <A href=href.clone() attr:class="block">
                    <div class="text-text font-medium">{title}</div>
                    <div class="faint text-xs mt-0.5">{format!("./{source_ref}")}</div>
                </A>
            </td>
            <td><span class="pill pill-neutral">{type_label}</span></td>
            <td><StatusPill label=status_label kind=status_kind /></td>
            <td class="text-right text-xs muted">{version_label}</td>
            <td class="text-right faint">"›"</td>
        </tr>
    }
}

fn ingest_status(doc: &DocumentListItemDto) -> (String, Status) {
    if doc.document_id.is_none() {
        ("Not ingested".to_string(), Status::Stale)
    } else if doc.indexings.is_empty() {
        ("Registered".to_string(), Status::Info)
    } else if doc.indexings.iter().any(|i| i.status.contains("Indexed")) {
        ("Indexed".to_string(), Status::Ok)
    } else if doc.indexings.iter().any(|i| i.status.contains("Failed")) {
        ("Failed".to_string(), Status::Fail)
    } else if doc.indexings.iter().any(|i| i.status.contains("Embedding")) {
        ("Embedding".to_string(), Status::Pending)
    } else if doc.indexings.iter().any(|i| i.status.contains("Chunking")) {
        ("Chunking".to_string(), Status::Pending)
    } else {
        ("Pending".to_string(), Status::Pending)
    }
}

fn document_type_label(doc_type: &str) -> &'static str {
    match doc_type {
        "BlogPost" => "Blog post",
        _ => "Document",
    }
}
