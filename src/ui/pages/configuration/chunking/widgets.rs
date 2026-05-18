use leptos::prelude::*;

#[component]
pub(super) fn StatusBanner(status: ReadSignal<Option<(bool, String)>>) -> impl IntoView {
    view! {
        {move || status.get().map(|(ok, msg)| {
            let cls = if ok {
                "surface mb-4 px-4 py-2"
            } else {
                "surface mb-4 px-4 py-2 log-line-error"
            };
            view! { <div class=cls>{msg}</div> }
        })}
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
pub(super) fn NumberField(
    label: String,
    hint: String,
    min: u32,
    #[prop(into)] value: Signal<u32>,
    on_change: Callback<u32>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                type="number"
                min=min.to_string()
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    let raw = event_target_value(&e);
                    if let Ok(parsed) = raw.parse::<u32>() {
                        let clamped = parsed.max(min);
                        on_change.run(clamped);
                    }
                }
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}
