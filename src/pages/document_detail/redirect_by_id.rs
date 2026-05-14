use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};
use leptos_router::NavigateOptions;
use uuid::Uuid;

use crate::components::primitives::{EmptyState, Surface};
use crate::server_functions::source_document::get_document_detail_by_id;

#[component]
pub fn DocumentByIdRedirect() -> impl IntoView {
    let params = use_params_map();
    let document_id = Memo::new(move |_| {
        params
            .with(|p| p.get("document_id").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });

    let detail = Resource::new(
        move || document_id.get(),
        move |id| async move {
            match id {
                Some(id) => get_document_detail_by_id(id)
                    .await
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    Effect::new(move |_| {
        if let Some(Ok(Some(detail))) = detail.get() {
            let target = format!(
                "/documents/{}/{}",
                detail.document.document_type, detail.document.source_ref_key,
            );
            use_navigate()(
                &target,
                NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
        }
    });

    view! {
        <Transition fallback=|| view! { <p class="muted">"Resolving document…"</p> }>
            {move || detail.get().map(|res| match res {
                Err(e) => view! {
                    <Surface><div class="log-line-error">{format!("Failed to resolve: {e}")}</div></Surface>
                }.into_any(),
                Ok(None) => view! {
                    <Surface>
                        <EmptyState
                            title="Document not found"
                            body="This document id is unknown or has been removed.".to_string()
                        />
                    </Surface>
                }.into_any(),
                Ok(Some(_)) => view! {
                    <p class="muted">"Redirecting…"</p>
                }.into_any(),
            })}
        </Transition>
    }
}
