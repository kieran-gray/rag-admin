use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use crate::server_functions::source_document::list_documents;
use crate::shared::contracts::{aggregate_type, DocumentListItemDto};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::Dialog;
use crate::ui::pages::shared::document_type_label;

#[component]
pub fn NewEvaluationDialog(
    #[prop(into)] open: Signal<bool>,
    on_close: Callback<()>,
) -> impl IntoView {
    let docs_invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });
    let docs = Resource::new(
        move || (open.get(), docs_invalidator.get()),
        |(is_open, _)| async move {
            if !is_open {
                return Ok(Vec::new());
            }
            list_documents().await
        },
    );

    let navigate = use_navigate();
    let go_to_doc = Callback::new(move |doc: DocumentListItemDto| {
        let href = format!(
            "/evaluate/{}/{}",
            doc.document_type.to_lowercase(),
            urlencoding::encode(&doc.source_ref_key),
        );
        on_close.run(());
        navigate(&href, NavigateOptions::default());
    });

    view! {
        <Dialog
            open=open
            title="Run a new evaluation"
            subtitle="Pick a document. The evaluation workflow opens at the dataset step.".to_string()
            on_close=on_close
        >
            <Suspense fallback=|| view! { <p class="muted text-sm">"Loading documents…"</p> }>
                {move || docs.get().map(|res| match res {
                    Err(e) => view! {
                        <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                    }.into_any(),
                    Ok(list) => {
                        let imported: Vec<DocumentListItemDto> = list
                            .into_iter()
                            .filter(|d| d.document_id.is_some())
                            .collect();
                        if imported.is_empty() {
                            view! {
                                <p class="muted text-sm">
                                    "No imported documents yet. Import one from the Documents page first."
                                </p>
                            }.into_any()
                        } else {
                            view! {
                                <ul class="flex flex-col gap-1">
                                    {imported.into_iter().map(|d| view! {
                                        <DocumentPickerRow doc=d on_pick=go_to_doc />
                                    }).collect_view()}
                                </ul>
                            }.into_any()
                        }
                    }
                })}
            </Suspense>
        </Dialog>
    }
}

#[component]
fn DocumentPickerRow(
    doc: DocumentListItemDto,
    on_pick: Callback<DocumentListItemDto>,
) -> impl IntoView {
    let title = doc.title.clone();
    let source_ref = doc.source_ref_key.clone();
    let type_label = document_type_label(&doc.document_type).to_string();
    let doc_for_click = doc.clone();

    view! {
        <li>
            <button
                type="button"
                class="w-full flex items-center justify-between gap-3 px-3 py-2 rounded text-left hover:bg-[var(--color-surface-hover)]"
                on:click=move |_| on_pick.run(doc_for_click.clone())
            >
                <div class="min-w-0">
                    <div class="text-sm truncate">{title}</div>
                    <div class="faint text-xs truncate">{format!("./{source_ref}")}</div>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    <span class="pill pill-neutral text-xs">{type_label}</span>
                    <span class="faint">"›"</span>
                </div>
            </button>
        </li>
    }
}
