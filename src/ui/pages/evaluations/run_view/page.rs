use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};
use uuid::Uuid;

use crate::server_functions::evaluation::get_run;
use crate::shared::contracts::{aggregate_type, EvaluationRunDto};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{EmptyState, Surface};

use super::compare_tab::CompareTabBody;
use super::header::RunHeader;
use super::progress_tab::ProgressTabBody;
use super::tabs::{default_tab_for, progress_tab_available, RunTab, RunTabs};
use super::variants_tab::VariantsTabBody;

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

    let tab_query = Memo::new(move |_| {
        query
            .with(|q| q.get("tab").map(|t| t.to_string()))
            .and_then(|s| RunTab::from_slug(&s))
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
                Ok(Some(r)) => view! {
                    <RunBody
                        run=r
                        active_tab=tab_query.get()
                        compare_with=compare_with.get()
                    />
                }.into_any(),
            })}
        </Transition>
    }
}

#[component]
fn RunBody(
    run: EvaluationRunDto,
    active_tab: Option<RunTab>,
    compare_with: Option<Uuid>,
) -> impl IntoView {
    let resolved_tab = active_tab.unwrap_or_else(|| {
        if compare_with.is_some() {
            RunTab::Compare
        } else {
            default_tab_for(&run)
        }
    });
    let progress_available = progress_tab_available(&run);
    let compare_available = compare_with.is_some();
    let active = match resolved_tab {
        RunTab::Progress if !progress_available => RunTab::Variants,
        RunTab::Compare if !compare_available => default_tab_for(&run),
        other => other,
    };

    let run_for_header = run.clone();
    let run_for_tab = run;

    view! {
        <div>
            <RunHeader run=run_for_header />
            <RunTabs
                run_id=run_for_tab.run_id
                active=active
                progress_available=progress_available
                compare_available=compare_available
            />
            <div class="pt-6">
                {match active {
                    RunTab::Progress => view! { <ProgressTabBody run=run_for_tab /> }.into_any(),
                    RunTab::Variants => view! { <VariantsTabBody run=run_for_tab /> }.into_any(),
                    RunTab::Compare => view! {
                        <CompareTabBody run=run_for_tab other_id=compare_with />
                    }.into_any(),
                }}
            </div>
        </div>
    }
}
