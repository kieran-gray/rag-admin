use leptos::prelude::*;

use super::dialog::Dialog;

#[component]
pub fn Help(
    #[prop(into)] title: String,
    #[prop(optional, into)] label: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let (open, set_open) = signal(false);
    let open_signal: Signal<bool> = open.into();
    let on_close = Callback::new(move |_| set_open.set(false));

    let children = StoredValue::new(children);
    let aria = label.unwrap_or_else(|| "More info".to_string());

    view! {
        <button
            type="button"
            class="help-icon"
            aria-label=aria
            on:click=move |_| set_open.set(true)
        >
            "?"
        </button>
        <Dialog open=open_signal title=title on_close=on_close>
            {move || children.with_value(|c| c())}
        </Dialog>
    }
}
