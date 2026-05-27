use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;

use crate::server_functions::configuration::get_configuration;
use crate::server_functions::embed::{embed_many, embed_matrix, embed_texts};
use crate::shared::contracts::EmbeddingModelDto;
use crate::shared::{EmbedInputType, EmbedManyResult, EmbedMatrixResult, EmbedResult};
use crate::ui::components::playground::RequestInspector;
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};

use super::history::{
    load_pairwise_history, save_pairwise_history, HistorySidebar, PairwiseHistoryEntry,
    PAIRWISE_HISTORY_LIMIT,
};
use super::matrix::{MatrixInputs, MatrixResultPanel};
use super::pairwise::{PairwiseInputs, PairwiseResultPanel};
use super::ranked::{RankedInputs, RankedResultPanel};
use super::shared::{EmbedMode, EmbeddingModelPicker, ModeToggle};

#[component]
pub fn EmbedPage() -> impl IntoView {
    let configuration = Resource::new(
        || (),
        |_| async move { get_configuration().await.map_err(|e| e.to_string()) },
    );

    view! {
        <div>
            <PageHeader
                title="Embed"
                subtitle="Compare texts via cosine similarity on a chosen embedding model.".to_string()
            />
            <Transition fallback=|| view! { <Surface><p class="muted">"Loading…"</p></Surface> }>
                {move || configuration.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error">{format!("Failed to load embedding models: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(cfg) if cfg.embedding_models.is_empty() => view! {
                        <Surface>
                            <EmptyState
                                title="No embedding models registered"
                                body="Add an embedding model to the Catalog before running similarity probes.".to_string()
                                action=Box::new(|| view! {
                                    <a class="btn" href="/configuration/catalog">"Open Catalog"</a>
                                }.into_any())
                            />
                        </Surface>
                    }.into_any(),
                    Ok(cfg) => view! { <EmbedBody models=cfg.embedding_models /> }.into_any(),
                })}
            </Transition>
        </div>
    }
}

#[component]
fn EmbedBody(models: Vec<EmbeddingModelDto>) -> impl IntoView {
    let query_params = use_query_map();
    let initial_a = query_params.with(|q| q.get("a").unwrap_or_default().to_string());
    let initial_b = query_params.with(|q| q.get("b").unwrap_or_default().to_string());
    let initial_model_param = query_params.with(|q| q.get("model").unwrap_or_default().to_string());

    let initial_model = if !initial_model_param.is_empty()
        && models.iter().any(|m| m.model == initial_model_param)
    {
        initial_model_param
    } else {
        models.first().map(|m| m.model.clone()).unwrap_or_default()
    };
    let models_stored = StoredValue::new(models);

    let mode = RwSignal::new(EmbedMode::Pairwise);
    let model = RwSignal::new(initial_model);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let last_request_json = RwSignal::new(String::new());
    let show_advanced = RwSignal::new(false);

    let text_a = RwSignal::new(initial_a);
    let text_b = RwSignal::new(initial_b);
    let type_a = RwSignal::new(EmbedInputType::Plain);
    let type_b = RwSignal::new(EmbedInputType::Plain);
    let pairwise_result = RwSignal::new(None::<EmbedResult>);
    let pairwise_history = RwSignal::new(load_pairwise_history());

    let ranked_query = RwSignal::new(String::new());
    let ranked_candidates = RwSignal::new(String::new());
    let ranked_query_type = RwSignal::new(EmbedInputType::Query);
    let ranked_candidate_type = RwSignal::new(EmbedInputType::Passage);
    let ranked_result = RwSignal::new(None::<EmbedManyResult>);

    let matrix_texts = RwSignal::new(String::new());
    let matrix_input_type = RwSignal::new(EmbedInputType::Plain);
    let matrix_result = RwSignal::new(None::<EmbedMatrixResult>);

    let inputs_disabled = Signal::derive(move || busy.get());

    let request_body_signal = Signal::derive(move || last_request_json.get());
    let has_request = Signal::derive(move || last_request_json.with(|s| !s.is_empty()));

    let swap_pair = move |_| {
        let a = text_a.get_untracked();
        let b = text_b.get_untracked();
        let ta = type_a.get_untracked();
        let tb = type_b.get_untracked();
        text_a.set(b);
        text_b.set(a);
        type_a.set(tb);
        type_b.set(ta);
    };

    let run_pairwise = move || {
        let m = model.get_untracked();
        let a = text_a.get_untracked();
        let b = text_b.get_untracked();
        if a.trim().is_empty() || b.trim().is_empty() {
            error.set(Some("Both text fields are required.".to_string()));
            return;
        }
        let ta = type_a.get_untracked();
        let tb = type_b.get_untracked();
        let preview = serde_json::json!({
            "endpoint": "embed_texts",
            "model": m.clone(),
            "type_a": ta.as_str(),
            "type_b": tb.as_str(),
            "text_a_chars": a.chars().count(),
            "text_b_chars": b.chars().count(),
        });
        last_request_json.set(serde_json::to_string_pretty(&preview).unwrap_or_default());
        busy.set(true);
        error.set(None);
        pairwise_result.set(None);
        let a_for_history = a.clone();
        let b_for_history = b.clone();
        let m_for_history = m.clone();
        spawn_local(async move {
            match embed_texts(m, a, b, ta, tb).await {
                Ok(r) => {
                    let sim = r.similarity;
                    pairwise_result.set(Some(r));
                    pairwise_history.update(|h| {
                        h.retain(|e| {
                            !(e.model == m_for_history
                                && e.text_a == a_for_history
                                && e.text_b == b_for_history)
                        });
                        h.insert(
                            0,
                            PairwiseHistoryEntry {
                                model: m_for_history,
                                text_a: a_for_history,
                                text_b: b_for_history,
                                similarity: sim,
                            },
                        );
                        h.truncate(PAIRWISE_HISTORY_LIMIT);
                        save_pairwise_history(h);
                    });
                }
                Err(e) => error.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let run_ranked = move || {
        let m = model.get_untracked();
        let q = ranked_query.get_untracked();
        let cs_text = ranked_candidates.get_untracked();
        if q.trim().is_empty() {
            error.set(Some("Query is required.".to_string()));
            return;
        }
        let candidates: Vec<String> = cs_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        if candidates.is_empty() {
            error.set(Some(
                "Add at least one candidate (one per line).".to_string(),
            ));
            return;
        }
        let qt = ranked_query_type.get_untracked();
        let ct = ranked_candidate_type.get_untracked();
        let preview = serde_json::json!({
            "endpoint": "embed_many",
            "model": m.clone(),
            "query_type": qt.as_str(),
            "candidate_type": ct.as_str(),
            "query_chars": q.chars().count(),
            "candidate_count": candidates.len(),
        });
        last_request_json.set(serde_json::to_string_pretty(&preview).unwrap_or_default());
        busy.set(true);
        error.set(None);
        ranked_result.set(None);
        spawn_local(async move {
            match embed_many(m, q, candidates, qt, ct).await {
                Ok(r) => ranked_result.set(Some(r)),
                Err(e) => error.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let run_matrix = move || {
        let m = model.get_untracked();
        let ts_text = matrix_texts.get_untracked();
        let texts: Vec<String> = ts_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        if texts.len() < 2 {
            error.set(Some(
                "Matrix mode needs at least 2 texts (one per line).".to_string(),
            ));
            return;
        }
        let t = matrix_input_type.get_untracked();
        let preview = serde_json::json!({
            "endpoint": "embed_matrix",
            "model": m.clone(),
            "input_type": t.as_str(),
            "text_count": texts.len(),
        });
        last_request_json.set(serde_json::to_string_pretty(&preview).unwrap_or_default());
        busy.set(true);
        error.set(None);
        matrix_result.set(None);
        spawn_local(async move {
            match embed_matrix(m, texts, t).await {
                Ok(r) => matrix_result.set(Some(r)),
                Err(e) => error.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let run = move |_| match mode.get_untracked() {
        EmbedMode::Pairwise => run_pairwise(),
        EmbedMode::Ranked => run_ranked(),
        EmbedMode::Matrix => run_matrix(),
    };

    let load_history_entry = move |entry: PairwiseHistoryEntry| {
        model.set(entry.model);
        text_a.set(entry.text_a);
        text_b.set(entry.text_b);
        mode.set(EmbedMode::Pairwise);
    };

    let clear_history = move || {
        pairwise_history.update(|h| {
            h.clear();
            save_pairwise_history(h);
        });
    };

    view! {
        <div class="playground-grid embed-grid">
            <div class="playground-main">
                <Surface
                    title="Inputs".to_string()
                    actions=Box::new(move || view! {
                        <div class="flex items-center gap-2">
                            <EmbeddingModelPicker
                                models=models_stored.get_value()
                                value=model
                                disabled=inputs_disabled
                            />
                        </div>
                    }.into_any())
                >
                    <div class="playground-body">
                        <ModeToggle current=mode disabled=inputs_disabled />

                        {move || match mode.get() {
                            EmbedMode::Pairwise => view! {
                                <PairwiseInputs
                                    text_a=text_a
                                    text_b=text_b
                                    type_a=type_a
                                    type_b=type_b
                                    disabled=inputs_disabled
                                    on_swap=swap_pair
                                />
                            }.into_any(),
                            EmbedMode::Ranked => view! {
                                <RankedInputs
                                    query=ranked_query
                                    candidates=ranked_candidates
                                    query_type=ranked_query_type
                                    candidate_type=ranked_candidate_type
                                    disabled=inputs_disabled
                                />
                            }.into_any(),
                            EmbedMode::Matrix => view! {
                                <MatrixInputs
                                    texts=matrix_texts
                                    input_type=matrix_input_type
                                    disabled=inputs_disabled
                                />
                            }.into_any(),
                        }}

                        <div class="embed-actions">
                            <button
                                type="button"
                                class="btn btn-primary"
                                disabled=move || busy.get()
                                    || model.with(String::is_empty)
                                on:click=run
                            >
                                {move || if busy.get() { "Embedding…" } else { "Run" }}
                            </button>
                        </div>

                        {move || error.get().map(|e| view! {
                            <div class="log-line-error">{e}</div>
                        })}

                        {move || has_request.get().then(|| view! {
                            <RequestInspector body=request_body_signal label="Inspect last request".to_string() />
                        })}
                    </div>
                </Surface>

                {move || match mode.get() {
                    EmbedMode::Pairwise => pairwise_result.get().map(|r| view! {
                        <PairwiseResultPanel result=r show_advanced=show_advanced />
                    }.into_any()).unwrap_or_else(|| ().into_any()),
                    EmbedMode::Ranked => ranked_result.get().map(|r| view! {
                        <RankedResultPanel result=r show_advanced=show_advanced />
                    }.into_any()).unwrap_or_else(|| ().into_any()),
                    EmbedMode::Matrix => matrix_result.get().map(|r| view! {
                        <MatrixResultPanel result=r />
                    }.into_any()).unwrap_or_else(|| ().into_any()),
                }}
            </div>

            <aside class="playground-sidebar">
                <HistorySidebar
                    history=pairwise_history
                    on_select=load_history_entry
                    on_clear=clear_history
                />
            </aside>
        </div>
    }
}
