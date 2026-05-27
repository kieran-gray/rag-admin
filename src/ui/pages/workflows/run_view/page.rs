use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};
use uuid::Uuid;

use crate::server_functions::evaluation::get_run;
use crate::shared::contracts::aggregate_type;
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, Surface};

use super::compare_tab::CompareTabBody;
use super::header::RunHeader;
use super::variants_tab::RunSinglePage;

#[component]
pub fn RunPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();

    let run_id = Memo::new(move |_| {
        params
            .with(|p| p.get("run_id").unwrap_or_default().to_string())
            .parse::<Uuid>()
            .ok()
    });

    let compare_with = Memo::new(move |_| {
        query
            .with(|q| q.get("with").map(|t| t.to_string()))
            .and_then(|s| Uuid::parse_str(&s).ok())
    });

    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::EVALUATION_RUN]));
    let run = Resource::new(
        move || (run_id.get(), invalidator.get()),
        move |(id, _)| async move {
            match id {
                Some(id) => get_run(id).await.map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    view! {
        <Transition fallback=|| view! { <p class="muted">"Loading run…"</p> }>
            {move || run.get().map(|res| match res {
                Err(e) => view! {
                    <Surface><div class="log-line-error">{format!("Failed to load: {e}")}</div></Surface>
                }.into_any(),
                Ok(None) => view! {
                    <Surface>
                        <EmptyState
                            title="Run not found"
                            body="This run id is unknown or has been removed.".to_string()
                        />
                    </Surface>
                }.into_any(),
                Ok(Some(r)) => {
                    let run_for_header = r.clone();
                    let run_for_body = r;
                    let with_id = compare_with.get();
                    view! {
                        <div>
                            <RunHeader run=run_for_header compare_with=with_id />
                            {match with_id {
                                Some(other) => view! {
                                    <CompareTabBody run=run_for_body other_id=Some(other) />
                                }.into_any(),
                                None => view! {
                                    <RunSinglePage run=run_for_body />
                                }.into_any(),
                            }}
                        </div>
                    }.into_any()
                }
            })}
        </Transition>
    }
}
