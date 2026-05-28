use leptos::prelude::*;

use crate::server_functions::configuration::discover_vector_indexes;
use crate::shared::contracts::{
    DiscoveredIndexDto, IndexDiscoveryOutcomeDto, IndexDiscoveryResponseDto,
};
use crate::shared::reference_data::VectorStoreKind;

use super::discovered_models_picker::{row_class, truncate};

#[component]
pub(super) fn DiscoveredIndexesPicker(
    kind: ReadSignal<VectorStoreKind>,
    index_name: ReadSignal<String>,
    on_select: Callback<DiscoveredIndexDto>,
) -> impl IntoView {
    let (refresh_tick, set_refresh_tick) = signal(0u32);

    let resource = Resource::new(
        move || (kind.get(), refresh_tick.get()),
        |(k, _)| async move { discover_vector_indexes(k).await.map_err(|e| e.to_string()) },
    );

    let refresh = move |_| {
        set_refresh_tick.update(|n| *n = n.wrapping_add(1));
    };

    view! {
        {move || (kind.get() == VectorStoreKind::CloudflareVectorize).then(|| view! {
            <div class="surface-raised rounded p-3 space-y-2">
                <div class="flex items-center justify-between gap-2">
                    <span class="eyebrow">"Available on Vectorize"</span>
                    <button type="button" class="btn btn-ghost btn-xs" on:click=refresh>
                        "Refresh"
                    </button>
                </div>

                <Suspense fallback=|| view! {
                    <div class="muted text-xs py-2">"Loading indexes…"</div>
                }>
                    {move || resource.get().map(|res| match res {
                        Err(message) => view! {
                            <div class="log-line-error text-xs py-2">{message}</div>
                        }.into_any(),
                        Ok(response) => render_outcome(response, index_name, on_select),
                    })}
                </Suspense>
            </div>
        })}
    }
}

fn render_outcome(
    response: IndexDiscoveryResponseDto,
    index_name: ReadSignal<String>,
    on_select: Callback<DiscoveredIndexDto>,
) -> AnyView {
    match response.outcome {
        IndexDiscoveryOutcomeDto::Success { indexes } => view! {
            <IndexList indexes=indexes index_name=index_name on_select=on_select />
        }
        .into_any(),
        IndexDiscoveryOutcomeDto::Failure { message } => view! {
            <div class="log-line-error text-xs py-2">
                {format!("Couldn't list Vectorize indexes — {message}")}
            </div>
        }
        .into_any(),
        IndexDiscoveryOutcomeDto::NotImplemented => view! {
            <div class="muted text-xs py-2">
                "Discovery isn't available for this store — enter the index name below."
            </div>
        }
        .into_any(),
    }
}

#[component]
fn IndexList(
    indexes: Vec<DiscoveredIndexDto>,
    index_name: ReadSignal<String>,
    on_select: Callback<DiscoveredIndexDto>,
) -> impl IntoView {
    if indexes.is_empty() {
        return view! {
            <div class="muted text-xs py-2">
                "No Vectorize indexes found — create one in Cloudflare, then refresh, or enter its name below."
            </div>
        }
        .into_any();
    }

    view! {
        <ul class="flex flex-col gap-1 max-h-56 overflow-y-auto">
            {indexes.into_iter().map(|i| {
                let name = i.name.clone();
                let i_for_select = i.clone();
                let subtitle = index_subtitle(&i);
                view! {
                    <li>
                        <button
                            type="button"
                            class=move || row_class(&name, &index_name.get())
                            on:click=move |_| on_select.run(i_for_select.clone())
                        >
                            <div class="text-sm text-text">{i.name.clone()}</div>
                            <div class="text-xs muted">{subtitle}</div>
                        </button>
                    </li>
                }
            }).collect_view()}
        </ul>
    }
    .into_any()
}

fn index_subtitle(i: &DiscoveredIndexDto) -> String {
    let mut parts = vec![format!("{}d", i.dimensions)];
    if let Some(metric) = i.metric.as_deref() {
        parts.push(metric.to_string());
    }
    if let Some(desc) = i.description.as_deref() {
        if !desc.is_empty() {
            parts.push(truncate(desc, 80));
        }
    }
    parts.join(" · ")
}
