use leptos::prelude::*;

#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn FilterChip(
    #[prop(into)] label: String,
    #[prop(into)] color: String,
    #[prop(into)] active: Signal<bool>,
    #[prop(optional)] count: Option<u64>,
    #[prop(optional)] show_remove: bool,
    on_click: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class="list-page-chip"
            data-active=move || active.get().to_string()
            on:click=move |_| on_click.run(())
        >
            <span class="list-page-chip-dot" style=format!("background-color: {color}")></span>
            <span>{label}</span>
            {count.map(|c| view! { <span class="list-page-chip-count">{c.to_string()}</span> })}
            {show_remove.then(|| view! { <span class="faint">"×"</span> })}
        </button>
    }
}
