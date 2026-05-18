mod chunk_step;
mod document_step;
mod embed_step;
mod index_step;
mod selection;

pub use chunk_step::ChunkStep;
pub use document_step::DocumentStep;
pub use embed_step::EmbedStep;
pub use index_step::IndexStep;
pub use selection::ConfigSelection;

use leptos::prelude::*;

#[derive(Clone)]
pub struct PipelineMetaDto {
    pub name: String,
    pub embedding_model: Option<String>,
    pub vector_index: Option<String>,
}

pub fn pipeline_meta_view(
    meta: Option<PipelineMetaDto>,
    document_version: u32,
    target_kind: &'static str,
    target: Option<String>,
) -> impl IntoView {
    let name = meta.map_or_else(|| "·".to_string(), |m| m.name);

    view! {
        <div class="flex items-center gap-3 flex-wrap text-sm">
            <span class="eyebrow">"Pipeline"</span>
            <span class="font-medium">{name}</span>
            <span class="muted">{format!("· doc v{document_version}")}</span>
            {target.map(|t| view! {
                <>
                    <span class="faint">"·"</span>
                    <span class="eyebrow">{target_kind}</span>
                    <span class="font-mono text-xs">{t}</span>
                </>
            })}
        </div>
    }
}
