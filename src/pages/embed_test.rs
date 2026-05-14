use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_query_map;

use crate::server_functions::embed::embed_texts;
use crate::shared::EmbedResult;

#[component]
pub fn EmbedTestPage() -> impl IntoView {
    let query = use_query_map();
    let initial_a = query.with(|q| q.get("a").unwrap_or_default().to_string());
    let initial_b = query.with(|q| q.get("b").unwrap_or_default().to_string());

    let (model, set_model) = signal("qwen3-embedding:0.6b".to_string());
    let (text_a, set_text_a) = signal(initial_a);
    let (text_b, set_text_b) = signal(initial_b);
    let (result, set_result) = signal::<Option<EmbedResult>>(None);
    let (error, set_error) = signal::<Option<String>>(None);
    let (loading, set_loading) = signal(false);

    let compute = move |_| {
        let m = model.get_untracked();
        let a = text_a.get_untracked();
        let b = text_b.get_untracked();

        if a.trim().is_empty() || b.trim().is_empty() {
            set_error.set(Some("Both text fields are required.".to_string()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);
        set_result.set(None);

        spawn_local(async move {
            match embed_texts(m, a, b).await {
                Ok(r) => {
                    set_result.set(Some(r));
                    set_loading.set(false);
                }
                Err(e) => {
                    set_error.set(Some(format!("EMBED_FAULT: {e}")));
                    set_loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="space-y-8">
            <div class="px-6 flex flex-col gap-1">
                <span class="tech-label opacity-40">"SYSTEM_VIEW / EMBED_TEST"</span>
                <h1 class="text-3xl font-bold tracking-tight">"SIMILARITY_LAB"</h1>
            </div>

            <div class="border-y border-[var(--color-border)] bg-[var(--color-card-inner)]/30">
                <div class="px-6 py-6 flex flex-col sm:flex-row gap-6 items-end">
                    <label class="block flex-1 space-y-2">
                        <div class="tech-label opacity-70">"MODEL_ID"</div>
                        <input
                            class="input font-mono text-sm"
                            prop:value=model
                            on:input=move |e| set_model.set(event_target_value(&e))
                        />
                        <div class="tech-label text-[9px] opacity-40 italic">
                            "> e.g. qwen3-embedding:0.6b, @cf/baai/bge-base-en-v1.5"
                        </div>
                    </label>
                    <button
                        class="btn btn-primary px-8 h-[42px]"
                        disabled=move || loading.get()
                        on:click=compute
                    >
                        {move || if loading.get() { "COMPUTING..." } else { "EXECUTE_TEST" }}
                    </button>
                </div>
            </div>

            <div class="grid grid-cols-1 lg:grid-cols-2 gap-px bg-[var(--color-border)] border-y border-x border-[var(--color-border)]">
                <div class="bg-[var(--color-page-bg)] px-6 py-6 space-y-3">
                    <div class="flex items-center gap-2">
                        <span class="tech-label">"01"</span>
                        <span class="tech-label opacity-50">"SOURCE_A"</span>
                    </div>
                    <textarea
                        class="input font-mono text-sm h-64 resize-y bg-black/20"
                        placeholder="Enter text segment A..."
                        prop:value=text_a
                        on:input=move |e| set_text_a.set(event_target_value(&e))
                    />
                </div>
                <div class="bg-[var(--color-page-bg)] px-6 py-6 space-y-3">
                    <div class="flex items-center gap-2">
                        <span class="tech-label">"02"</span>
                        <span class="tech-label opacity-50">"SOURCE_B"</span>
                    </div>
                    <textarea
                        class="input font-mono text-sm h-64 resize-y bg-black/20"
                        placeholder="Enter text segment B..."
                        prop:value=text_b
                        on:input=move |e| set_text_b.set(event_target_value(&e))
                    />
                </div>
            </div>

            <div class="px-6">
                {move || {
                    error
                        .get()
                        .map(|e| {
                            view! {
                                <div class="card-outer p-4 log-line-error font-mono text-xs bg-red-950/20">{e}</div>
                            }
                        })
                }}
            </div>

            {move || {
                result
                    .get()
                    .map(|r| {
                        let EmbedResult { dims, norm_a, norm_b, similarity } = r;
                        view! {
                            <div class="border-y border-[var(--color-border)] overflow-hidden">
                                <div class="px-6 py-8 border-b border-[var(--color-border)] bg-[var(--color-card-inner)]/20">
                                    <div class="flex flex-col items-center text-center">
                                        <span class="tech-label opacity-40 mb-4">"COSINE_SIMILARITY_SCORE"</span>
                                        <span class=format!(
                                            "text-7xl font-mono font-bold tracking-tighter {}",
                                            similarity_class(similarity),
                                        )>{format!("{:.4}", similarity)}</span>
                                        <span class=format!(
                                            "tech-label mt-4 px-3 py-1 border border-current {}",
                                            similarity_class(similarity),
                                        )>{similarity_label(similarity)}</span>
                                    </div>
                                </div>

                                <div class="grid grid-cols-1 md:grid-cols-3 divide-y md:divide-y-0 md:divide-x divide-[var(--color-border)]">
                                    <div class="px-6 py-6 bg-[var(--color-card-inner)]/10">
                                        <div class="tech-label opacity-40 mb-2">"VECTOR_DIMENSIONS"</div>
                                        <div class="font-mono text-xl font-bold">
                                            {dims.to_string()}
                                        </div>
                                    </div>
                                    <div class="px-6 py-6 bg-[var(--color-card-inner)]/10">
                                        <div class="tech-label opacity-40 mb-2">"L2_NORM_A"</div>
                                        <div class=format!(
                                            "font-mono text-xl font-bold {}",
                                            norm_class(norm_a),
                                        )>{format!("{:.4}", norm_a)}</div>
                                    </div>
                                    <div class="px-6 py-6 bg-[var(--color-card-inner)]/10">
                                        <div class="tech-label opacity-40 mb-2">"L2_NORM_B"</div>
                                        <div class=format!(
                                            "font-mono text-xl font-bold {}",
                                            norm_class(norm_b),
                                        )>{format!("{:.4}", norm_b)}</div>
                                    </div>
                                </div>

                                <div class="px-6 py-4 bg-black/40 border-t border-[var(--color-border)]">
                                    <p class="text-[10px] font-mono opacity-40 leading-relaxed text-center italic">
                                        "PROTIP: Well-behaved models normalize to ~1.0. Significant deviation suggests misconfiguration."
                                    </p>
                                </div>
                            </div>
                        }
                    })
            }}
        </div>
    }
}

fn similarity_class(s: f32) -> &'static str {
    if s >= 0.85 {
        "log-line-success"
    } else if s >= 0.65 {
        "text-amber-400"
    } else if s >= 0.45 {
        ""
    } else {
        "log-line-error"
    }
}

fn similarity_label(s: f32) -> &'static str {
    if s >= 0.85 {
        "HIGHLY_SIMILAR"
    } else if s >= 0.65 {
        "RELATED"
    } else if s >= 0.45 {
        "LOOSELY_RELATED"
    } else {
        "UNRELATED"
    }
}

fn norm_class(n: f32) -> &'static str {
    if (n - 1.0).abs() < 0.1 {
        "log-line-success"
    } else {
        "text-amber-400"
    }
}
