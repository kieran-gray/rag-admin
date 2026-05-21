use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;

use crate::server_functions::configuration::get_configuration;
use crate::server_functions::embed::embed_texts;
use crate::shared::contracts::EmbeddingModelDto;
use crate::shared::EmbedResult;
use crate::ui::components::primitives::{EmptyState, MetricBar, MetricKind, PageHeader, Surface};

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
                subtitle="Compare two text segments via cosine similarity on a chosen embedding model.".to_string()
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
    let query = use_query_map();
    let initial_a = query.with(|q| q.get("a").unwrap_or_default().to_string());
    let initial_b = query.with(|q| q.get("b").unwrap_or_default().to_string());

    let initial_model = models.first().map(|m| m.model.clone()).unwrap_or_default();
    let models_stored = StoredValue::new(models);

    let (model, set_model) = signal(initial_model);
    let (text_a, set_text_a) = signal(initial_a);
    let (text_b, set_text_b) = signal(initial_b);
    let (result, set_result) = signal::<Option<EmbedResult>>(None);
    let (error, set_error) = signal::<Option<String>>(None);
    let (busy, set_busy) = signal(false);

    let compute = move |_| {
        let m = model.get_untracked();
        let a = text_a.get_untracked();
        let b = text_b.get_untracked();

        if a.trim().is_empty() || b.trim().is_empty() {
            set_error.set(Some("Both text fields are required.".to_string()));
            return;
        }

        set_busy.set(true);
        set_error.set(None);
        set_result.set(None);

        spawn_local(async move {
            match embed_texts(m, a, b).await {
                Ok(r) => set_result.set(Some(r)),
                Err(e) => set_error.set(Some(e.to_string())),
            }
            set_busy.set(false);
        });
    };

    let inputs_disabled = move || busy.get();

    view! {
        <div class="space-y-4">
            <Surface
                title="Inputs".to_string()
                actions=Box::new(move || view! {
                    <EmbeddingModelPicker
                        models=models_stored.get_value()
                        value=model
                        set_value=set_model
                        disabled=Signal::derive(inputs_disabled)
                    />
                }.into_any())
            >
                <div class="embed-sources">
                    <label class="block space-y-1.5">
                        <span class="eyebrow">"Text A"</span>
                        <textarea
                            class="playground-query-input"
                            placeholder="First text segment…"
                            rows="8"
                            disabled=inputs_disabled
                            prop:value=move || text_a.get()
                            on:input=move |e| set_text_a.set(event_target_value(&e))
                        ></textarea>
                    </label>
                    <label class="block space-y-1.5">
                        <span class="eyebrow">"Text B"</span>
                        <textarea
                            class="playground-query-input"
                            placeholder="Second text segment…"
                            rows="8"
                            disabled=inputs_disabled
                            prop:value=move || text_b.get()
                            on:input=move |e| set_text_b.set(event_target_value(&e))
                        ></textarea>
                    </label>
                </div>

                <div class="playground-controls">
                    <div class="playground-control-spacer"></div>
                    <button
                        type="button"
                        class="btn btn-primary"
                        disabled=move || busy.get()
                            || text_a.with(|t| t.trim().is_empty())
                            || text_b.with(|t| t.trim().is_empty())
                            || model.with(String::is_empty)
                        on:click=compute
                    >
                        {move || if busy.get() { "Embedding…" } else { "Run" }}
                    </button>
                </div>

                {move || error.get().map(|e| view! {
                    <div class="log-line-error mt-3">{e}</div>
                })}
            </Surface>

            {move || result.get().map(|r| view! { <ResultPanel result=r /> })}
        </div>
    }
}

#[component]
fn EmbeddingModelPicker(
    models: Vec<EmbeddingModelDto>,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
    disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        <label class="playground-picker-label">
            <span class="eyebrow">"Model"</span>
            <select
                class="playground-profile-picker"
                disabled=disabled
                on:change=move |ev| set_value.set(event_target_value(&ev))
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
fn ResultPanel(result: EmbedResult) -> impl IntoView {
    let EmbedResult {
        dims,
        norm_a,
        norm_b,
        similarity,
    } = result;

    let similarity_clamped = similarity.clamp(0.0, 1.0);
    let bucket = similarity_bucket(similarity);

    view! {
        <Surface title=format!("Similarity · {similarity:.3}")>
            <MetricBar
                label="Cosine similarity".to_string()
                short=bucket.label.to_string()
                value=similarity_clamped
                kind=bucket.kind
            />

            <div class="embed-stats">
                <Stat label="Dimensions".to_string() value=dims.to_string() />
                <Stat label="‖A‖".to_string() value=format!("{norm_a:.4}") tone=norm_tone(norm_a) />
                <Stat label="‖B‖".to_string() value=format!("{norm_b:.4}") tone=norm_tone(norm_b) />
            </div>

            <p class="text-xs faint mt-3">
                "Well-behaved models normalize to ~1.0; large deviations suggest a model or transport mismatch."
            </p>
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
    if (n - 1.0).abs() < 0.1 {
        ""
    } else {
        "log-line-warn"
    }
}
