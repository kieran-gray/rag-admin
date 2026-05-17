use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use uuid::Uuid;

use crate::contracts::{
    IndexingDto, PipelineConfigurationDto, QueryHit, QueryRequest, QueryResult,
};
use crate::server_functions::query::query_documents;
use crate::ui::components::primitives::Surface;

#[component]
pub fn SearchSection(
    document_id: Uuid,
    indexings: Vec<IndexingDto>,
    pipelines: Vec<PipelineConfigurationDto>,
) -> impl IntoView {
    let pipelines_stored = StoredValue::new(pipelines);
    let any_indexed = indexings
        .iter()
        .any(|ix| ix.status.contains("Indexed") && !ix.removed);
    let indexings_stored = StoredValue::new(indexings);

    view! {
        <section class="space-y-4">
            <h2 class="flex items-center gap-2 text-base font-semibold">
                <span class="inline-flex items-center justify-center w-6 h-6 rounded-full text-xs font-mono"
                      style="background: var(--color-accent-soft); color: var(--color-accent)">
                    "2"
                </span>
                <span>"Search this document"</span>
            </h2>

            {if any_indexed {
                view! {
                    <SearchPane
                        document_id=document_id
                        indexings=indexings_stored
                        pipelines=pipelines_stored
                    />
                }.into_any()
            } else {
                view! {
                    <Surface>
                        <p class="muted text-sm p-2">
                            "Run an indexing above to enable search."
                        </p>
                    </Surface>
                }.into_any()
            }}
        </section>
    }
}

#[component]
fn SearchPane(
    document_id: Uuid,
    indexings: StoredValue<Vec<IndexingDto>>,
    pipelines: StoredValue<Vec<PipelineConfigurationDto>>,
) -> impl IntoView {
    let indexed_pipeline_ids: Vec<Uuid> = indexings.with_value(|ixs| {
        ixs.iter()
            .filter(|ix| ix.status.contains("Indexed") && !ix.removed)
            .map(|ix| ix.pipeline_configuration_id)
            .collect()
    });
    let default_pipeline_id = pipelines.with_value(|ps| {
        indexed_pipeline_ids
            .iter()
            .find(|id| {
                ps.iter()
                    .any(|p| p.pipeline_configuration_id == **id && p.is_default)
            })
            .or_else(|| indexed_pipeline_ids.first())
            .copied()
    });

    let (pipeline_id, set_pipeline_id) = signal::<Option<Uuid>>(default_pipeline_id);
    let (query, set_query) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (result, set_result) = signal::<Option<QueryResult>>(None);
    let (error, set_error) = signal::<Option<String>>(None);

    let pipeline_choices: Vec<(Uuid, String)> = pipelines.with_value(|ps| {
        ps.iter()
            .filter(|p| indexed_pipeline_ids.contains(&p.pipeline_configuration_id))
            .map(|p| {
                let suffix = if p.is_default { " · default" } else { "" };
                (p.pipeline_configuration_id, format!("{}{}", p.name, suffix))
            })
            .collect()
    });
    let pipeline_choices_stored = StoredValue::new(pipeline_choices);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let q = query.get_untracked().trim().to_string();
        if q.is_empty() {
            return;
        }
        let Some(pid) = pipeline_id.get_untracked() else {
            return;
        };
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let req = QueryRequest {
                pipeline_configuration_id: pid,
                query: q,
                top_k: 5,
                min_score: 0.0,
                document_id: Some(document_id),
            };
            match query_documents(req).await {
                Ok(res) => {
                    set_result.set(Some(res));
                    set_busy.set(false);
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(format!("{e}")));
                }
            }
        });
    };

    let playground_link = move || {
        pipeline_id.get().map(|pid| {
            let q = query.get();
            let q_enc = urlencoding::encode(&q);
            format!("/playground/retrieve?q={q_enc}&pipeline={pid}")
        })
    };

    view! {
        <Surface>
            <form on:submit=submit class="space-y-3">
                <div class="flex gap-2">
                    <input
                        type="text"
                        class="input flex-1"
                        placeholder="Search this document…"
                        prop:value=query
                        on:input=move |ev| set_query.set(event_target_value(&ev))
                    />
                    <button type="submit" class="btn btn-primary" disabled=busy>
                        {move || if busy.get() { "Searching…" } else { "Search" }}
                    </button>
                </div>

                {move || {
                    let choices = pipeline_choices_stored.get_value();
                    if choices.len() > 1 {
                        view! {
                            <label class="flex items-center gap-2 text-xs muted">
                                <span>"Pipeline"</span>
                                <select
                                    class="input text-xs py-1"
                                    on:change=move |ev| {
                                        set_pipeline_id.set(Uuid::parse_str(&event_target_value(&ev)).ok());
                                    }
                                >
                                    {choices.into_iter().map(|(id, label)| {
                                        let selected = pipeline_id.get() == Some(id);
                                        view! {
                                            <option value=id.to_string() selected=selected>{label}</option>
                                        }
                                    }).collect_view()}
                                </select>
                            </label>
                        }.into_any()
                    } else {
                        ().into_any()
                    }
                }}
            </form>

            {move || error.get().map(|e| view! {
                <div class="log-line-error text-sm mt-3">{e}</div>
            })}

            {move || result.get().map(|res| view! { <SearchResults result=res /> })}

            {move || playground_link().map(|href| view! {
                <div class="pt-3 text-xs">
                    <A href=href attr:class="muted hover:text-text underline-offset-2 hover:underline">
                        "Open in Playground →"
                    </A>
                </div>
            })}
        </Surface>
    }
}

#[component]
fn SearchResults(result: QueryResult) -> impl IntoView {
    let hits = result.hits;
    view! {
        <div class="mt-4 space-y-2">
            {if hits.is_empty() {
                view! {
                    <p class="muted text-sm">"No matches in this document."</p>
                }.into_any()
            } else {
                view! {
                    <ul class="divide-y divide-[var(--color-border)]">
                        {hits.into_iter().map(|h| view! { <HitRow hit=h /> }).collect_view()}
                    </ul>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn HitRow(hit: QueryHit) -> impl IntoView {
    let score = format!("{:.3}", hit.score);
    let heading = hit.heading.unwrap_or_default();
    let snippet = hit.snippet;
    view! {
        <li class="py-2 space-y-1">
            <div class="flex items-center gap-2 text-xs muted">
                <span class="font-mono">{score}</span>
                {(!heading.is_empty()).then(|| view! {
                    <>
                        <span class="faint">"·"</span>
                        <span>{heading}</span>
                    </>
                })}
            </div>
            <p class="text-sm">{snippet}</p>
        </li>
    }
}
