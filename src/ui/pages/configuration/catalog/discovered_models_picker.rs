use leptos::prelude::*;

use crate::server_functions::configuration::discover_models;
use crate::shared::contracts::{DiscoveredModelDto, DiscoveryOutcomeDto, DiscoveryResponseDto};
use crate::shared::reference_data::{AiProviderKind, ModelCapability};

const FILTER_THRESHOLD: usize = 6;

#[component]
pub(super) fn DiscoveredModelsPicker(
    provider: ReadSignal<AiProviderKind>,
    capability: ModelCapability,
    show_dimensions: bool,
    model_id: ReadSignal<String>,
    on_select: Callback<DiscoveredModelDto>,
) -> impl IntoView {
    let (show_all, set_show_all) = signal(false);
    let (refresh_tick, set_refresh_tick) = signal(0u32);

    let resource = Resource::new(
        move || (provider.get(), capability, refresh_tick.get()),
        |(p, c, _)| async move { discover_models(p, c).await.map_err(|e| e.to_string()) },
    );

    let header_title = move || match provider.get() {
        AiProviderKind::Cloudflare => "Available on Cloudflare",
        AiProviderKind::Ollama => "Pulled on Ollama",
        AiProviderKind::LlamaServer => "Available on llama-server",
    };

    let not_implemented = move || {
        matches!(
            resource.get(),
            Some(Ok(r)) if matches!(r.outcome, DiscoveryOutcomeDto::NotImplemented)
        )
    };

    let refresh = move |_| {
        set_refresh_tick.update(|n| *n = n.wrapping_add(1));
    };

    view! {
        {move || (provider.get() != AiProviderKind::LlamaServer).then(|| view! {
            <div class="surface-raised rounded p-3 space-y-2">
                <div class="flex items-center justify-between gap-2">
                    <span class="eyebrow">{move || header_title().to_string()}</span>
                    <div class="flex items-center gap-3 text-xs">
                        {move || (provider.get() == AiProviderKind::Ollama).then(|| view! {
                            <label class="flex items-center gap-1 muted">
                                <input
                                    type="checkbox"
                                    prop:checked=move || show_all.get()
                                    on:change=move |e| set_show_all.set(event_target_checked(&e))
                                />
                                "Show all"
                            </label>
                        })}
                        {move || (!not_implemented()).then(|| view! {
                            <button type="button" class="btn btn-ghost btn-xs" on:click=refresh>
                                "Refresh"
                            </button>
                        })}
                    </div>
                </div>

                <Suspense fallback=|| view! {
                    <div class="muted text-xs py-2">"Loading models…"</div>
                }>
                    {move || resource.get().map(|res| match res {
                        Err(message) => view! {
                            <PickerErrorLine message=message />
                        }.into_any(),
                        Ok(response) => render_outcome(
                            response,
                            capability,
                            show_dimensions,
                            show_all,
                            model_id,
                            on_select,
                        ).into_any(),
                    })}
                </Suspense>
            </div>
        })}
    }
}

fn render_outcome(
    response: DiscoveryResponseDto,
    capability: ModelCapability,
    show_dimensions: bool,
    show_all: ReadSignal<bool>,
    model_id: ReadSignal<String>,
    on_select: Callback<DiscoveredModelDto>,
) -> impl IntoView {
    match response.outcome {
        DiscoveryOutcomeDto::Success { models } => view! {
            <PickerList
                provider=response.provider
                capability=capability
                show_dimensions=show_dimensions
                show_all=show_all
                models=models
                model_id=model_id
                on_select=on_select
            />
        }
        .into_any(),
        DiscoveryOutcomeDto::Failure { message } => view! {
            <PickerErrorLine message=failure_copy(response.provider, &message) />
        }
        .into_any(),
        DiscoveryOutcomeDto::NotImplemented => view! {
            <div class="muted text-xs py-2">
                "Discovery isn't available for this provider yet — enter the model identifier below."
            </div>
        }
        .into_any(),
    }
}

fn failure_copy(provider: AiProviderKind, message: &str) -> String {
    let lead = match provider {
        AiProviderKind::Cloudflare => "Couldn't list Cloudflare models",
        AiProviderKind::Ollama => "Couldn't list Ollama models",
        AiProviderKind::LlamaServer => "Couldn't list llama-server models",
    };
    format!("{lead} — {message}")
}

#[component]
fn PickerErrorLine(message: String) -> impl IntoView {
    view! {
        <div class="log-line-error text-xs py-2">{message}</div>
    }
}

#[component]
fn PickerList(
    provider: AiProviderKind,
    capability: ModelCapability,
    show_dimensions: bool,
    show_all: ReadSignal<bool>,
    models: Vec<DiscoveredModelDto>,
    model_id: ReadSignal<String>,
    on_select: Callback<DiscoveredModelDto>,
) -> impl IntoView {
    let total = models.len();
    let stored_models = StoredValue::new(models);
    let (filter, set_filter) = signal(String::new());

    let visible = move || {
        let all = show_all.get();
        let query = filter.get();
        stored_models.with_value(|list| {
            list.iter()
                .filter(|m| all || matches_capability(m, capability))
                .filter(|m| matches_filter(m, &query))
                .cloned()
                .collect::<Vec<_>>()
        })
    };

    let stored_id_present = move || {
        let id = model_id.get();
        if id.is_empty() {
            return true;
        }
        stored_models.with_value(|list| list.iter().any(|m| m.id == id))
    };

    view! {
        {(total > FILTER_THRESHOLD).then(|| view! {
            <input
                class="input text-sm"
                placeholder="Filter models…"
                prop:value=move || filter.get()
                on:input=move |e| set_filter.set(event_target_value(&e))
            />
        })}
        {move || {
            let rows = visible();
            if rows.is_empty() {
                let message = if filter.get().is_empty() {
                    empty_copy(provider, total).to_string()
                } else {
                    "No models match your filter.".to_string()
                };
                view! {
                    <div class="muted text-xs py-2">{message}</div>
                }.into_any()
            } else {
                view! {
                    <ul class="flex flex-col gap-1 max-h-56 overflow-y-auto">
                        {rows.into_iter().map(|m| {
                            let id = m.id.clone();
                            let m_for_select = m.clone();
                            let row_title = m.display_name.clone().unwrap_or_else(|| m.id.clone());
                            let subtitle = model_subtitle(&m, show_dimensions);
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        class=move || row_class(&id, &model_id.get())
                                        on:click=move |_| on_select.run(m_for_select.clone())
                                    >
                                        <div class="text-sm text-text">{row_title}</div>
                                        {(!subtitle.is_empty()).then(|| view! {
                                            <div class="text-xs muted">{subtitle.clone()}</div>
                                        })}
                                    </button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }
        }}
        {move || (!stored_id_present()).then(|| {
            let id = model_id.get();
            view! {
                <div class="text-xs muted pt-1">
                    {format!("Current value \u{201C}{id}\u{201D} is not in the discovery list — clicking a row replaces it, or keep typing in Model ID below.")}
                </div>
            }
        })}
    }
}

pub(super) fn row_class(row_id: &str, current_id: &str) -> &'static str {
    if row_id == current_id {
        "w-full text-left rounded px-2 py-1.5 surface-raised border border-[var(--color-accent)]"
    } else {
        "w-full text-left rounded px-2 py-1.5 hover:bg-[var(--color-surface-hover)] border border-transparent"
    }
}

fn empty_copy(provider: AiProviderKind, total: usize) -> &'static str {
    match (provider, total) {
        (_, 0) => match provider {
            AiProviderKind::Cloudflare => {
                "No Cloudflare models available for this task — register one by name below."
            }
            AiProviderKind::Ollama => {
                "No models pulled on Ollama yet — register one by name below."
            }
            AiProviderKind::LlamaServer => "No models reported by llama-server.",
        },
        _ => "No models match the current capability — toggle \"Show all\" to see everything.",
    }
}

fn matches_capability(m: &DiscoveredModelDto, capability: ModelCapability) -> bool {
    match m.capability_hint {
        Some(hint) => hint == capability,
        None => true,
    }
}

fn matches_filter(m: &DiscoveredModelDto, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_ascii_lowercase();
    m.id.to_ascii_lowercase().contains(&query)
        || m.display_name
            .as_deref()
            .is_some_and(|d| d.to_ascii_lowercase().contains(&query))
}

pub(super) fn model_subtitle(m: &DiscoveredModelDto, show_dimensions: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if show_dimensions {
        if let Some(d) = m.dimensions {
            parts.push(format!("{d}d"));
        }
    }
    if let Some(ctx) = m.context_length {
        parts.push(format!("{} ctx", humanise_context(ctx)));
    }
    if let Some(ps) = m.parameter_size.as_deref() {
        parts.push(ps.to_string());
    }
    if let Some(q) = m.quantization.as_deref() {
        parts.push(q.to_string());
    }
    if let Some(size) = m.size_bytes {
        parts.push(humanise_size(size));
    }
    if let Some(desc) = m.description.as_deref() {
        let truncated = truncate(desc, 80);
        parts.push(truncated);
    }
    parts.join(" · ")
}

fn humanise_context(ctx: u32) -> String {
    if ctx >= 1024 && ctx.is_multiple_of(1024) {
        format!("{}k", ctx / 1024)
    } else {
        ctx.to_string()
    }
}

fn humanise_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub(super) fn truncate(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        s.to_string()
    } else {
        let prefix: String = s.chars().take(n).collect();
        format!("{prefix}\u{2026}")
    }
}
