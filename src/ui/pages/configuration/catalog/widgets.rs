use leptos::prelude::*;

use crate::shared::reference_data::{AiProviderKind, VectorStoreKind};
use crate::ui::components::primitives::InlineStatusMessage;

#[component]
pub(super) fn StatusBanner(status: ReadSignal<Option<InlineStatusMessage>>) -> impl IntoView {
    view! {
        {move || status.get().map(|m| {
            let cls = if m.ok {
                "surface mb-4 px-4 py-2"
            } else {
                "surface mb-4 px-4 py-2 log-line-error"
            };
            view! { <div class=cls>{m.text}</div> }
        })}
    }
}

#[component]
pub(super) fn AiKindSelect(
    value: ReadSignal<AiProviderKind>,
    set_value: WriteSignal<AiProviderKind>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">"Provider"</span>
            <select
                class="input"
                on:change=move |e| {
                    let v = event_target_value(&e);
                    let kind = AiProviderKind::all()
                        .iter()
                        .copied()
                        .find(|k| k.as_str() == v)
                        .unwrap_or(AiProviderKind::Cloudflare);
                    set_value.set(kind);
                }
            >
                {AiProviderKind::all().iter().copied().map(|k| {
                    let key = k.as_str();
                    let label = k.display_label();
                    view! {
                        <option value=key selected=move || value.get() == k>{label}</option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
pub(super) fn VectorKindSelect(
    value: ReadSignal<VectorStoreKind>,
    set_value: WriteSignal<VectorStoreKind>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">"Vector store"</span>
            <select
                class="input"
                on:change=move |e| {
                    let v = event_target_value(&e);
                    let kind = VectorStoreKind::all()
                        .iter()
                        .copied()
                        .find(|k| k.as_str() == v)
                        .unwrap_or(VectorStoreKind::CloudflareVectorize);
                    set_value.set(kind);
                }
            >
                {VectorStoreKind::all().iter().copied().map(|k| {
                    let key = k.as_str();
                    let label = k.display_label();
                    view! {
                        <option value=key selected=move || value.get() == k>{label}</option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
pub(super) fn LabelledInput(
    label: String,
    hint: String,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                prop:value=move || value.get()
                on:input=move |e| set_value.set(event_target_value(&e))
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

#[component]
pub(super) fn LabelledNum(
    label: String,
    hint: String,
    value: ReadSignal<u32>,
    set_value: WriteSignal<u32>,
    #[prop(default = 0)] min: u32,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                type="number"
                min=min
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let v: u32 = event_target_value(&e).parse().unwrap_or(min);
                    set_value.set(v.max(min));
                }
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}
