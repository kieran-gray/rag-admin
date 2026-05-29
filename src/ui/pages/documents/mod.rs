pub mod add_dialog;
pub mod all_view;
pub mod connector_pull;
pub mod coverage;
pub mod redirect_by_id;
pub mod smart_filter;
pub mod source_mark;

pub use connector_pull::ConnectorPullPage;
pub use redirect_by_id::DocumentByIdRedirect;

use leptos::prelude::*;

use crate::ui::components::primitives::PageHeader;
use crate::ui::pages::documents::add_dialog::AddDialog;

use self::all_view::AllDocumentsView;

#[component]
pub fn DocumentsPage() -> impl IntoView {
    let (dialog_open, set_dialog_open) = signal(false);
    let open_signal: Signal<bool> = dialog_open.into();
    let close_dialog = Callback::new(move |_| set_dialog_open.set(false));
    let open_import = Callback::new(move |_| set_dialog_open.set(true));

    view! {
        <div>
            <PageHeader
                title="Documents"
                subtitle="Your corpus: every imported document, filterable by source, status, and connector.".to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=move |_| set_dialog_open.set(true)
                    >
                        "+ Add documents"
                    </button>
                }.into_any())
            />

            <AllDocumentsView open_import=open_import />

            <AddDialog open=open_signal on_close=close_dialog />
        </div>
    }
}
