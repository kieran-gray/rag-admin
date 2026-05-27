use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;

use crate::server_functions::configuration::get_configuration;
use crate::server_functions::embed::{embed_many, embed_matrix, embed_texts};
use crate::shared::contracts::EmbeddingModelDto;
use crate::shared::{EmbedInputType, EmbedManyResult, EmbedMatrixResult, EmbedResult};
use crate::ui::components::playground::{LatencyBadge, RequestInspector};
use crate::ui::components::primitives::{EmptyState, MetricBar, MetricKind, PageHeader, Surface};

#[cfg(feature = "hydrate")]
const PAIRWISE_HISTORY_KEY: &str = "rag-admin:embed-history";
const PAIRWISE_HISTORY_LIMIT: usize = 8;
const TOKEN_WARN_CHARS: usize = 30_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EmbedMode {
    Pairwise,
    Ranked,
    Matrix,
}

impl EmbedMode {
    fn label(self) -> &'static str {
        match self {
            Self::Pairwise => "Pairwise",
            Self::Ranked => "Ranked",
            Self::Matrix => "Matrix",
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PairwiseHistoryEntry {
    model: String,
    text_a: String,
    text_b: String,
    similarity: f32,
}

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

    // Pairwise state
    let text_a = RwSignal::new(initial_a);
    let text_b = RwSignal::new(initial_b);
    let type_a = RwSignal::new(EmbedInputType::Plain);
    let type_b = RwSignal::new(EmbedInputType::Plain);
    let pairwise_result = RwSignal::new(None::<EmbedResult>);
    let pairwise_history = RwSignal::new(load_pairwise_history());

    // Ranked state
    let ranked_query = RwSignal::new(String::new());
    let ranked_candidates = RwSignal::new(String::new());
    let ranked_query_type = RwSignal::new(EmbedInputType::Query);
    let ranked_candidate_type = RwSignal::new(EmbedInputType::Passage);
    let ranked_result = RwSignal::new(None::<EmbedManyResult>);

    // Matrix state
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
            error.set(Some("Matrix mode needs at least 2 texts (one per line).".to_string()));
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

    let clear_history = move |_| {
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
                <Surface
                    title="Recent pairs".to_string()
                    actions=Box::new(move || view! {
                        <button
                            type="button"
                            class="btn btn-sm btn-ghost"
                            disabled=move || pairwise_history.with(Vec::is_empty)
                            on:click=clear_history
                        >
                            "Clear"
                        </button>
                    }.into_any())
                >
                    {move || {
                        let h = pairwise_history.get();
                        if h.is_empty() {
                            view! {
                                <p class="muted text-sm">"No history yet."</p>
                            }.into_any()
                        } else {
                            h.into_iter().map(|entry| {
                                let entry_for_click = entry.clone();
                                let preview_a = preview(&entry.text_a, 40);
                                let preview_b = preview(&entry.text_b, 40);
                                let sim = format!("{:.3}", entry.similarity);
                                view! {
                                    <button
                                        type="button"
                                        class="playground-history-row"
                                        on:click=move |_| load_history_entry(entry_for_click.clone())
                                    >
                                        <div class="playground-history-query">
                                            <span class="embed-history-sim">{sim}</span>
                                            " "
                                            {preview_a}
                                        </div>
                                        <div class="playground-history-meta">
                                            <span>{preview_b}</span>
                                        </div>
                                    </button>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                </Surface>
            </aside>
        </div>
    }
}

#[component]
fn ModeToggle(current: RwSignal<EmbedMode>, disabled: Signal<bool>) -> impl IntoView {
    let modes = [EmbedMode::Pairwise, EmbedMode::Ranked, EmbedMode::Matrix];
    view! {
        <div class="embed-mode-toggle" role="tablist">
            {modes.into_iter().map(|m| {
                let label = m.label();
                let is_current = Memo::new(move |_| current.get() == m);
                view! {
                    <button
                        type="button"
                        role="tab"
                        class=move || if is_current.get() { "embed-mode-btn embed-mode-btn-active" } else { "embed-mode-btn" }
                        disabled=disabled
                        on:click=move |_| current.set(m)
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn InputTypeSelect(
    value: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
) -> impl IntoView {
    let options = [
        EmbedInputType::Plain,
        EmbedInputType::Query,
        EmbedInputType::Passage,
    ];
    view! {
        <select
            class="embed-input-type"
            disabled=disabled
            on:change=move |ev| {
                let v = event_target_value(&ev);
                let kind = match v.as_str() {
                    "query" => EmbedInputType::Query,
                    "passage" => EmbedInputType::Passage,
                    _ => EmbedInputType::Plain,
                };
                value.set(kind);
            }
        >
            {options.into_iter().map(|opt| {
                let label = opt.label();
                let opt_str = opt.as_str();
                view! {
                    <option value=opt_str selected=move || value.get() == opt>{label}</option>
                }
            }).collect_view()}
        </select>
    }
}

#[component]
fn CharCount(text: RwSignal<String>) -> impl IntoView {
    let count = Memo::new(move |_| text.with(|t| t.chars().count()));
    let warn = Memo::new(move |_| count.get() > TOKEN_WARN_CHARS);
    let class = move || if warn.get() {
        "embed-char-count embed-char-count-warn"
    } else {
        "embed-char-count"
    };
    view! {
        <span class=class title="char count · approx tokens">
            {move || {
                let c = count.get();
                let tokens = c.div_ceil(4);
                format!("{c} chars · ≈{tokens} tok")
            }}
        </span>
    }
}

#[component]
fn PairwiseInputs(
    text_a: RwSignal<String>,
    text_b: RwSignal<String>,
    type_a: RwSignal<EmbedInputType>,
    type_b: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
    on_swap: impl Fn(()) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="embed-sources">
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Text A"</span>
                    <InputTypeSelect value=type_a disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="First text segment…"
                    rows="8"
                    disabled=disabled
                    prop:value=move || text_a.get()
                    on:input=move |e| text_a.set(event_target_value(&e))
                ></textarea>
                <CharCount text=text_a />
            </label>
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Text B"</span>
                    <InputTypeSelect value=type_b disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="Second text segment…"
                    rows="8"
                    disabled=disabled
                    prop:value=move || text_b.get()
                    on:input=move |e| text_b.set(event_target_value(&e))
                ></textarea>
                <CharCount text=text_b />
            </label>
        </div>
        <div class="embed-swap-row">
            <button
                type="button"
                class="btn btn-sm btn-ghost"
                title="Swap A ↔ B (useful for asymmetric models)"
                disabled=disabled
                on:click=move |_| on_swap(())
            >
                "Swap A ↔ B"
            </button>
        </div>
    }
}

#[component]
fn RankedInputs(
    query: RwSignal<String>,
    candidates: RwSignal<String>,
    query_type: RwSignal<EmbedInputType>,
    candidate_type: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <div class="embed-sources">
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Query"</span>
                    <InputTypeSelect value=query_type disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="What are you searching for?"
                    rows="4"
                    disabled=disabled
                    prop:value=move || query.get()
                    on:input=move |e| query.set(event_target_value(&e))
                ></textarea>
                <CharCount text=query />
            </label>
            <label class="embed-source-field">
                <div class="embed-source-head">
                    <span class="eyebrow">"Candidates (one per line)"</span>
                    <InputTypeSelect value=candidate_type disabled=disabled />
                </div>
                <textarea
                    class="playground-query-input"
                    placeholder="Paste one candidate per line…"
                    rows="10"
                    disabled=disabled
                    prop:value=move || candidates.get()
                    on:input=move |e| candidates.set(event_target_value(&e))
                ></textarea>
                <CharCount text=candidates />
            </label>
        </div>
    }
}

#[component]
fn MatrixInputs(
    texts: RwSignal<String>,
    input_type: RwSignal<EmbedInputType>,
    disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <label class="embed-source-field">
            <div class="embed-source-head">
                <span class="eyebrow">"Texts (one per line, max 16)"</span>
                <InputTypeSelect value=input_type disabled=disabled />
            </div>
            <textarea
                class="playground-query-input"
                placeholder="Paste one text per line…"
                rows="10"
                disabled=disabled
                prop:value=move || texts.get()
                on:input=move |e| texts.set(event_target_value(&e))
            ></textarea>
            <CharCount text=texts />
        </label>
    }
}

#[component]
fn EmbeddingModelPicker(
    models: Vec<EmbeddingModelDto>,
    value: RwSignal<String>,
    disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <label class="playground-picker-label">
            <span class="eyebrow">"Model"</span>
            <select
                class="playground-profile-picker"
                disabled=disabled
                on:change=move |ev| value.set(event_target_value(&ev))
            >
                {models.into_iter().map(|m| {
                    let key = m.model.clone();
                    let value_match = key.clone();
                    let label = format!("{} · {}d · {}", m.model, m.dimensions, m.kind.display_label());
                    view! {
                        <option
                            value=key
                            selected=move || value.get() == value_match
                        >
                            {label}
                        </option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn PairwiseResultPanel(result: EmbedResult, show_advanced: RwSignal<bool>) -> impl IntoView {
    let EmbedResult {
        dims,
        norm_a,
        norm_b,
        similarity,
        timings,
    } = result;

    let similarity_clamped = similarity.clamp(0.0, 1.0);
    let bucket = similarity_bucket(similarity);
    let norm_warns = norm_warn(norm_a) || norm_warn(norm_b);
    if norm_warns {
        show_advanced.set(true);
    }

    view! {
        <Surface
            title=format!("Similarity · {similarity:.3}")
            actions=Box::new(move || view! { <LatencyBadge timings=timings /> }.into_any())
        >
            <div class="playground-body">
                <MetricBar
                    label="Cosine similarity".to_string()
                    short=bucket.label.to_string()
                    value=similarity_clamped
                    kind=bucket.kind
                />
                <ThresholdLegend />

                <details class="embed-advanced" prop:open=move || show_advanced.get()>
                    <summary on:click=move |_| {
                        let next = !show_advanced.get_untracked();
                        show_advanced.set(next);
                    }>
                        "Advanced · dims, norms"
                    </summary>
                    <div class="embed-stats">
                        <Stat label="Dimensions".to_string() value=dims.to_string() />
                        <Stat label="‖A‖".to_string() value=format!("{norm_a:.4}") tone=norm_tone(norm_a) />
                        <Stat label="‖B‖".to_string() value=format!("{norm_b:.4}") tone=norm_tone(norm_b) />
                    </div>
                    {norm_warns.then(|| view! {
                        <p class="text-xs faint mt-3">
                            "Well-behaved models normalize to ~1.0; large deviations suggest a model or transport mismatch."
                        </p>
                    })}
                </details>
            </div>
        </Surface>
    }
}

#[component]
fn RankedResultPanel(result: EmbedManyResult, show_advanced: RwSignal<bool>) -> impl IntoView {
    let EmbedManyResult {
        dims,
        query_norm,
        candidates,
        timings,
    } = result;

    let count = candidates.len();
    let candidates_stored = StoredValue::new(candidates);

    view! {
        <Surface
            title=format!("Ranked · {count}")
            actions=Box::new(move || view! { <LatencyBadge timings=timings /> }.into_any())
        >
            <div class="playground-body">
                <ThresholdLegend />
                <ol class="embed-ranked-list">
                    {candidates_stored.with_value(|cs| cs.iter().enumerate().map(|(i, c)| {
                        let sim = c.similarity.clamp(0.0, 1.0);
                        let bucket = similarity_bucket(c.similarity);
                        let preview = c.text_preview.clone();
                        let norm = c.norm;
                        view! {
                            <li class="embed-ranked-row">
                                <div class="embed-ranked-rank">{i + 1}</div>
                                <div class="embed-ranked-body">
                                    <div class="embed-ranked-text">{preview}</div>
                                    <MetricBar
                                        label=format!("{:.3}", c.similarity)
                                        short=bucket.label.to_string()
                                        value=sim
                                        kind=bucket.kind
                                    />
                                </div>
                                <div class="embed-ranked-norm" title="‖vec‖">{format!("{norm:.3}")}</div>
                            </li>
                        }
                    }).collect_view())}
                </ol>

                <details class="embed-advanced" prop:open=move || show_advanced.get()>
                    <summary on:click=move |_| {
                        let next = !show_advanced.get_untracked();
                        show_advanced.set(next);
                    }>
                        "Advanced · dims, query norm"
                    </summary>
                    <div class="embed-stats">
                        <Stat label="Dimensions".to_string() value=dims.to_string() />
                        <Stat
                            label="‖query‖".to_string()
                            value=format!("{query_norm:.4}")
                            tone=norm_tone(query_norm)
                        />
                    </div>
                </details>
            </div>
        </Surface>
    }
}

#[component]
fn MatrixResultPanel(result: EmbedMatrixResult) -> impl IntoView {
    let EmbedMatrixResult {
        dims,
        previews,
        norms,
        matrix,
        timings,
    } = result;

    let n = previews.len();
    let matrix_stored = StoredValue::new(matrix);
    let previews_stored = StoredValue::new(previews.clone());
    let norms_stored = StoredValue::new(norms);

    view! {
        <Surface
            title=format!("Matrix · {n}x{n}")
            actions=Box::new(move || view! { <LatencyBadge timings=timings /> }.into_any())
        >
            <div class="playground-body">
                <div
                    class="embed-matrix"
                    style=format!(
                        "grid-template-columns: minmax(140px, 1fr) repeat({n}, minmax(36px, 1fr));"
                    )
                >
                    <div class="embed-matrix-corner"></div>
                    {(0..n).map(|j| view! {
                        <div class="embed-matrix-col-head">{j + 1}</div>
                    }).collect_view()}

                    {(0..n).map(|i| {
                        let preview = previews_stored.with_value(|p| p.get(i).cloned().unwrap_or_default());
                        let norm = norms_stored.with_value(|ns| ns.get(i).copied().unwrap_or_default());
                        let row_head = view! {
                            <div class="embed-matrix-row-head" title=format!("‖vec‖ {norm:.3}")>
                                <span class="embed-matrix-row-rank">{i + 1}</span>
                                <span class="embed-matrix-row-text">{preview}</span>
                            </div>
                        };
                        let cells = (0..n).map(|j| {
                            let sim = matrix_stored.with_value(|m| {
                                m.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0.0)
                            });
                            let bg = matrix_cell_bg(sim);
                            view! {
                                <div
                                    class="embed-matrix-cell"
                                    style=format!("background-color: {bg};")
                                    title=format!("({}, {}) {sim:.3}", i + 1, j + 1)
                                >
                                    {format!("{sim:.2}")}
                                </div>
                            }
                        }).collect_view();
                        view! { <>{row_head}{cells}</> }
                    }).collect_view()}
                </div>

                <p class="text-xs faint mt-3">
                    {format!("Dimensions: {dims}")}
                </p>
            </div>
        </Surface>
    }
}

#[component]
fn Stat(label: String, value: String, #[prop(default = "")] tone: &'static str) -> impl IntoView {
    let value_class = if tone.is_empty() {
        "embed-stat-value".to_string()
    } else {
        format!("embed-stat-value {tone}")
    };
    view! {
        <div class="embed-stat">
            <div class="eyebrow">{label}</div>
            <div class=value_class>{value}</div>
        </div>
    }
}

#[component]
fn ThresholdLegend() -> impl IntoView {
    view! {
        <div class="embed-threshold-legend">
            <span class="embed-threshold-legend-stop" style="left: 45%">"0.45"</span>
            <span class="embed-threshold-legend-stop" style="left: 65%">"0.65"</span>
            <span class="embed-threshold-legend-stop" style="left: 85%">"0.85"</span>
            <span class="embed-threshold-legend-track" aria-hidden="true"></span>
            <div class="embed-threshold-legend-labels">
                <span class="faint">"unrelated"</span>
                <span class="faint">"loose"</span>
                <span class="faint">"related"</span>
                <span class="faint">"highly"</span>
            </div>
        </div>
    }
}

struct SimilarityBucket {
    label: &'static str,
    kind: MetricKind,
}

fn similarity_bucket(s: f32) -> SimilarityBucket {
    if s >= 0.85 {
        SimilarityBucket {
            label: "Highly similar",
            kind: MetricKind::Best,
        }
    } else if s >= 0.65 {
        SimilarityBucket {
            label: "Related",
            kind: MetricKind::Default,
        }
    } else if s >= 0.45 {
        SimilarityBucket {
            label: "Loosely related",
            kind: MetricKind::Default,
        }
    } else {
        SimilarityBucket {
            label: "Unrelated",
            kind: MetricKind::Default,
        }
    }
}

fn norm_tone(n: f32) -> &'static str {
    if norm_warn(n) {
        "log-line-warn"
    } else {
        ""
    }
}

fn norm_warn(n: f32) -> bool {
    (n - 1.0).abs() >= 0.1
}

fn matrix_cell_bg(sim: f32) -> String {
    let s = sim.clamp(0.0, 1.0);
    let alpha = 0.08_f32 + 0.7_f32 * s;
    format!("rgba(247, 118, 142, {alpha:.3})")
}

fn preview(text: &str, max: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let mut out: String = collapsed.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(feature = "hydrate")]
fn load_pairwise_history() -> Vec<PairwiseHistoryEntry> {
    let Some(window) = web_sys::window() else {
        return Vec::new();
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return Vec::new();
    };
    let Ok(Some(raw)) = storage.get_item(PAIRWISE_HISTORY_KEY) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

#[cfg(feature = "hydrate")]
fn save_pairwise_history(entries: &[PairwiseHistoryEntry]) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    if entries.is_empty() {
        drop(storage.remove_item(PAIRWISE_HISTORY_KEY));
        return;
    }
    if let Ok(json) = serde_json::to_string(entries) {
        drop(storage.set_item(PAIRWISE_HISTORY_KEY, &json));
    }
}

#[cfg(not(feature = "hydrate"))]
fn load_pairwise_history() -> Vec<PairwiseHistoryEntry> {
    Vec::new()
}

#[cfg(not(feature = "hydrate"))]
fn save_pairwise_history(_entries: &[PairwiseHistoryEntry]) {}
