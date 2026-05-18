use leptos::prelude::*;

use crate::ui::pages::document_detail::tabs::SourceTab;

#[component]
pub fn DocumentStep(source_ref: String, on_advance: Callback<()>) -> impl IntoView {
    view! {
        <div class="space-y-6">
            <SourceTab source_ref=source_ref />

            <div class="step-advance">
                <div class="step-advance-eyebrow">
                    <span>"Next"</span>
                    <span class="step-advance-eyebrow-label">"Chunk this document"</span>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    on:click=move |_| on_advance.run(())
                >
                    "Continue to chunking →"
                </button>
            </div>
        </div>
    }
}
