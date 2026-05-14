use crate::shared::ChunkDto;
use leptos::prelude::*;

#[component]
pub fn ChunkCard(chunk: ChunkDto) -> impl IntoView {
    let text_length = chunk.text.len();
    let heading = chunk.heading.clone();
    let sequence = chunk.sequence;
    let text = StoredValue::new(chunk.text);

    view! {
        <div class="surface-raised rounded p-3 flex flex-col gap-2">
            <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                    <div class="eyebrow">{format!("Chunk · seq {:03}", sequence)}</div>
                    <div class="text-sm font-medium truncate">{heading}</div>
                </div>
                <span class="text-xs muted shrink-0">{format!("{text_length} chars")}</span>
            </div>
            <pre class="text-xs leading-relaxed whitespace-pre-wrap max-h-40 overflow-auto muted">
                {text.get_value()}
            </pre>
            <div class="flex justify-end pt-1 border-t border-[var(--color-border)]">
                <a
                    href=text.with_value(|t| format!("/embed?a={}", urlencoding::encode(t)))
                    class="text-xs muted hover:text-text"
                >
                    "Probe similarity →"
                </a>
            </div>
        </div>
    }
}
