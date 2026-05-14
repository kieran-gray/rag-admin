use leptos::prelude::*;

use super::chunk_card::ChunkCard;
use crate::components::primitives::{EmptyState, Surface};
use crate::server_functions::source_document::get_chunks;
use crate::shared::SourceDocumentDetailDto;

#[component]
pub fn ChunksTab(detail: Option<SourceDocumentDetailDto>) -> impl IntoView {
    let indexings = detail.map(|d| d.indexings).unwrap_or_default();

    let with_chunks: Vec<_> = indexings
        .into_iter()
        .filter(|ix| ix.chunk_set_id.is_some())
        .collect();

    let (selected, set_selected) = signal::<Option<uuid::Uuid>>(None);

    let chunks = Resource::new(
        move || selected.get(),
        move |cid| async move {
            match cid {
                Some(id) => get_chunks(id).await.map_err(|e| e.to_string()),
                None => Ok(vec![]),
            }
        },
    );

    let with_chunks_stored = StoredValue::new(with_chunks);

    view! {
        <div class="space-y-6">
            <Surface title="Indexing".to_string()>
                {move || {
                    let list = with_chunks_stored.get_value();
                    if list.is_empty() {
                        return view! {
                            <EmptyState
                                title="No chunked indexings yet"
                                body="Run an indexing first; chunks appear here once chunking completes.".to_string()
                            />
                        }.into_any();
                    }
                    view! {
                        <div class="flex gap-2 flex-wrap">
                            {list.into_iter().map(|ix| {
                                let cid = ix.chunk_set_id.unwrap();
                                let active = move || selected.get() == Some(cid);
                                let pipeline_short =
                                    ix.pipeline_configuration_id.to_string()[..8].to_string();
                                view! {
                                    <button
                                        type="button"
                                        class=move || format!(
                                            "px-3 py-1.5 rounded border text-sm font-mono transition-colors {}",
                                            if active() {
                                                "border-[var(--color-accent)] text-[var(--color-accent)]"
                                            } else {
                                                "border-[var(--color-border)] muted hover:text-text"
                                            }
                                        )
                                        on:click=move |_| set_selected.set(Some(cid))
                                    >
                                        {format!("pipeline:{pipeline_short}… (v{})", ix.document_version)}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </Surface>

            <Transition fallback=|| view! {
                <Surface><p class="muted">"Loading chunks…"</p></Surface>
            }>
                {move || chunks.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(cs) if cs.is_empty() && selected.get().is_some() => view! {
                        <Surface>
                            <EmptyState
                                title="No chunks"
                                body="This indexing has no chunks yet.".to_string()
                            />
                        </Surface>
                    }.into_any(),
                    Ok(cs) if cs.is_empty() => ().into_any(),
                    Ok(cs) => view! {
                        <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                            {cs.into_iter().map(|c| view! { <ChunkCard chunk=c /> }).collect_view()}
                        </div>
                    }.into_any(),
                })}
            </Transition>
        </div>
    }
}
