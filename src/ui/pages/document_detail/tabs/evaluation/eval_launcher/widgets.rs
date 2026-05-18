use leptos::prelude::*;

use crate::shared::reference_data::ChunkStrategy;

#[component]
pub(super) fn ModeRadio<T>(
    value: ReadSignal<T>,
    set_value: WriteSignal<T>,
    options: Vec<(T, &'static str)>,
) -> impl IntoView
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    view! {
        <div class="flex gap-1.5 flex-wrap">
            {options.into_iter().map(|(target, label)| {
                let active = move || value.get() == target;
                view! {
                    <button
                        type="button"
                        class=move || format!(
                            "px-3 py-1.5 rounded border text-sm transition-colors {}",
                            if active() {
                                "border-[var(--color-accent)] text-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                            } else {
                                "border-[var(--color-border)] muted hover:text-text"
                            }
                        )
                        on:click=move |_| set_value.set(target)
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
pub(super) fn StrategyPicker(
    value: ReadSignal<ChunkStrategy>,
    set_value: WriteSignal<ChunkStrategy>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2">
            <span class="eyebrow">"Strategy"</span>
            <ModeRadio<ChunkStrategy>
                value=value
                set_value=set_value
                options=vec![
                    (ChunkStrategy::Section, "section"),
                    (ChunkStrategy::Bert, "bert"),
                    (ChunkStrategy::Llm, "llm"),
                    (ChunkStrategy::Darn, "darn"),
                ]
            />
        </div>
    }
}

#[component]
pub(super) fn FieldRow(children: Children) -> impl IntoView {
    view! { <div class="flex flex-wrap gap-3">{children()}</div> }
}

#[component]
pub(super) fn NumField(
    label: String,
    value: ReadSignal<u32>,
    set_value: WriteSignal<u32>,
    #[prop(default = 0)] min: u32,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 min-w-32">
            <span class="eyebrow">{label}</span>
            <input
                class="input font-mono"
                type="number"
                min=min
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let v: u32 = event_target_value(&e).parse().unwrap_or(min);
                    set_value.set(v.max(min));
                }
            />
        </label>
    }
}

#[component]
pub(super) fn TextField(
    label: String,
    hint: &'static str,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 flex-1 min-w-48">
            <span class="eyebrow">{label}</span>
            <input
                class="input font-mono"
                type="text"
                placeholder=hint
                prop:value=move || value.get()
                on:input=move |e| set_value.set(event_target_value(&e))
            />
        </label>
    }
}
