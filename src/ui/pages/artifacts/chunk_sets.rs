use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::server_functions::chunk_set::{
    delete_chunk_set, gc_chunk_sets, get_chunk_sets, set_chunk_set_pinned,
};
use crate::shared::contracts::{
    ChunkSetSummaryDto, DeleteChunkSetRequestDto, GcChunkSetsRequestDto,
    SetChunkSetPinnedRequestDto,
};
use crate::ui::components::primitives::{
    EmptyState, InlineStatus, InlineStatusMessage, PageHeader, Surface,
};

#[component]
pub fn ChunkSetsPage() -> impl IntoView {
    let (refresh, set_refresh) = signal(0u32);
    let chunk_sets = Resource::new(
        move || refresh.get(),
        |_| async move { get_chunk_sets().await.map_err(|e| e.to_string()) },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);
    let (gc_days, set_gc_days) = signal::<u32>(7);

    let bump = move || set_refresh.update(|n| *n = n.wrapping_add(1));

    let on_pin = Callback::new(move |(id, pinned): (uuid::Uuid, bool)| {
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match set_chunk_set_pinned(SetChunkSetPinnedRequestDto {
                chunk_set_id: id,
                pinned,
            })
            .await
            {
                Ok(_) => {
                    set_status.set(Some(InlineStatusMessage::ok(if pinned {
                        "Pinned"
                    } else {
                        "Unpinned"
                    })));
                    bump();
                }
                Err(e) => {
                    set_status.set(Some(InlineStatusMessage::err(e.to_string())));
                }
            }
            set_busy.set(false);
        });
    });

    let on_delete = Callback::new(move |id: uuid::Uuid| {
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match delete_chunk_set(DeleteChunkSetRequestDto { chunk_set_id: id }).await {
                Ok(_) => {
                    set_status.set(Some(InlineStatusMessage::ok("Chunk set deleted")));
                    bump();
                }
                Err(e) => {
                    set_status.set(Some(InlineStatusMessage::err(e.to_string())));
                }
            }
            set_busy.set(false);
        });
    });

    let on_gc = move |_| {
        let days = gc_days.get_untracked();
        let secs = (days as u64).saturating_mul(86_400);
        set_busy.set(true);
        set_status.set(None);
        spawn_local(async move {
            match gc_chunk_sets(GcChunkSetsRequestDto {
                older_than_seconds: secs,
            })
            .await
            {
                Ok(resp) => {
                    set_status.set(Some(InlineStatusMessage::ok(format!(
                        "Deleted {} unused chunk set{}",
                        resp.deleted,
                        if resp.deleted == 1 { "" } else { "s" },
                    ))));
                    bump();
                }
                Err(e) => {
                    set_status.set(Some(InlineStatusMessage::err(e.to_string())));
                }
            }
            set_busy.set(false);
        });
    };

    view! {
        <div>
            <PageHeader
                title="Chunk sets"
                subtitle="Cached chunkings of documents reused across indexing and evaluation runs. Pin to protect from cleanup; in-use chunk sets cannot be deleted directly.".to_string()
            />

            <InlineStatus status=status />

            <Surface title="Clean up".to_string() class="mb-2">
                <div class="flex items-center gap-3">
                    <label class="text-sm muted">"Delete unused chunk sets older than"</label>
                    <input
                        type="number"
                        min="0"
                        class="input w-20"
                        prop:value=move || gc_days.get().to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                set_gc_days.set(v);
                            }
                        }
                    />
                    <span class="text-sm muted">"days"</span>
                    <button
                        type="button"
                        class="btn btn-primary text-nowrap"
                        prop:disabled=move || busy.get()
                        on:click=on_gc
                    >
                        "Run cleanup"
                    </button>
                </div>
                <p class="text-xs muted mt-2">
                    "Skips pinned chunk sets and any referenced by an active indexing or evaluation run."
                </p>
            </Surface>

            <Suspense fallback=|| view! {
                <Surface><div class="p-6 muted text-sm">"Loading chunk sets…"</div></Surface>
            }>
                {move || chunk_sets.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(list) if list.is_empty() => view! {
                        <Surface>
                            <EmptyState
                                title="No chunk sets"
                                body="Chunk sets are created when you index a document or run an evaluation. They are reused across runs that share the same chunking config.".to_string()
                            />
                        </Surface>
                    }.into_any(),
                    Ok(list) => view! {
                        <div class="space-y-2">
                            {list.into_iter().map(|cs| view! {
                                <ChunkSetRow
                                    cs=cs
                                    busy=busy
                                    on_pin=on_pin
                                    on_delete=on_delete
                                />
                            }).collect_view()}
                        </div>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn ChunkSetRow(
    cs: ChunkSetSummaryDto,
    busy: ReadSignal<bool>,
    on_pin: Callback<(uuid::Uuid, bool)>,
    on_delete: Callback<uuid::Uuid>,
) -> impl IntoView {
    let id = cs.chunk_set_id;
    let pinned = cs.pinned;
    let in_use = cs.in_use();
    let strategy = cs.chunking_config.strategy().as_str();
    let short_id = id.to_string().chars().take(8).collect::<String>();
    let when = cs.created_at.get(..16).unwrap_or(&cs.created_at).to_string();
    let chunk_count = cs.chunk_count;
    let indexing_refs = cs.indexing_refs;
    let variant_result_refs = cs.variant_result_refs;
    let doc_short = cs.document_id.to_string().chars().take(8).collect::<String>();

    view! {
        <Surface>
            <div class="flex items-center justify-between gap-3">
                <div class="flex items-center gap-3 min-w-0">
                    <span class="font-mono text-sm">{format!("cs-{short_id}")}</span>
                    <span class="text-sm muted">{format!("doc-{doc_short} v{}", cs.document_version)}</span>
                    <span class="pill">{strategy}</span>
                    <span class="text-xs muted">{format!("{chunk_count} chunks")}</span>
                    {(indexing_refs > 0).then(|| view! {
                        <span class="pill pill-ok">
                            {format!("indexed × {indexing_refs}")}
                        </span>
                    })}
                    {(variant_result_refs > 0).then(|| view! {
                        <span class="pill pill-ok">
                            {format!("eval × {variant_result_refs}")}
                        </span>
                    })}
                    {pinned.then(|| view! {
                        <span class="pill pill-ok">"📌 pinned"</span>
                    })}
                </div>
                <div class="flex items-center gap-2">
                    <span class="text-xs faint font-mono">{when}</span>
                    <button
                        type="button"
                        class="btn btn-ghost"
                        prop:disabled=move || busy.get()
                        on:click=move |_| on_pin.run((id, !pinned))
                    >
                        {if pinned { "Unpin" } else { "Pin" }}
                    </button>
                    <button
                        type="button"
                        class="btn btn-ghost"
                        title=if in_use { "In use — pinned or referenced; cannot delete" } else { "Delete" }
                        prop:disabled=move || busy.get() || in_use
                        on:click=move |_| on_delete.run(id)
                    >
                        "Delete"
                    </button>
                </div>
            </div>
        </Surface>
    }
}
