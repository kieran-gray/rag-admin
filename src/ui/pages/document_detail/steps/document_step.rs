use leptos::prelude::*;

use crate::contracts::{
    ChunkingConfigurationDto, PipelineConfigurationDto, SourceDocumentDetailDto, SweepTemplateDto,
};
use crate::ui::components::primitives::Surface;
use crate::ui::pages::document_detail::tabs::{EvaluationTab, SourceTab};

#[component]
pub fn DocumentStep(
    detail: SourceDocumentDetailDto,
    pipelines: Vec<PipelineConfigurationDto>,
    chunking_configurations: Vec<ChunkingConfigurationDto>,
    sweep_templates: Vec<SweepTemplateDto>,
    source_ref: String,
    on_advance: Callback<()>,
) -> impl IntoView {
    let (show_evaluate, set_show_evaluate) = signal(false);
    let detail_for_eval = StoredValue::new(detail);
    let pipelines_for_eval = StoredValue::new(pipelines);
    let chunking_for_eval = StoredValue::new(chunking_configurations);
    let sweep_for_eval = StoredValue::new(sweep_templates);

    view! {
        <div class="space-y-6">
            <Surface title="Not sure how to chunk it?">
                <div class="flex items-start justify-between gap-4 flex-wrap">
                    <div class="space-y-1 max-w-2xl">
                        <h3 class="text-sm font-semibold"></h3>
                        <p class="text-sm muted">
                            "Generate a synthetic Q&A set and let the optimizer score chunking strategies for this document. Recommended if you haven't indexed a document like this before."
                        </p>
                    </div>
                    <button
                        type="button"
                        class=move || if show_evaluate.get() { "btn".to_string() } else { "btn btn-primary".to_string() }
                        on:click=move |_| set_show_evaluate.update(|v| *v = !*v)
                    >
                        {move || if show_evaluate.get() { "Hide tuning" } else { "Find best chunking" }}
                    </button>
                </div>

                {move || show_evaluate.get().then(|| view! {
                    <div class="mt-4 pt-4 border-t border-[var(--color-border)]">
                        <EvaluationTab
                            detail=Some(detail_for_eval.get_value())
                            pipelines=pipelines_for_eval.get_value()
                            chunking_configurations=chunking_for_eval.get_value()
                            sweep_templates=sweep_for_eval.get_value()
                        />
                    </div>
                })}
            </Surface>

            <details class="surface collapsible-card" open>
                <summary class="collapsible-card-summary">
                    <span class="section-title">"Source markdown"</span>
                    <span class="muted text-xs collapsible-card-hint">
                        <span class="when-open">"Click to collapse"</span>
                        <span class="when-closed">"Click to expand"</span>
                        <span class="collapsible-card-chevron">"▾"</span>
                    </span>
                </summary>
                <div class="collapsible-card-body">
                    <p class="text-sm muted mb-4">
                        "This is the markdown we'll chunk and embed. Review it, then continue to chunking."
                    </p>
                    <SourceTab source_ref=source_ref />
                </div>
            </details>

            <div class="flex justify-end pt-2">
                <button
                    type="button"
                    class="btn btn-primary"
                    on:click=move |_| on_advance.run(())
                >
                    "Proceed to chunking →"
                </button>
            </div>
        </div>
    }
}
