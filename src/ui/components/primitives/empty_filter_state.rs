use leptos::prelude::*;

#[component]
pub fn EmptyFilterState(
    #[prop(into)] title: String,
    #[prop(into)] body: String,
    on_clear: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="list-page-empty-filter">
            <strong>{title}</strong>
            <p class="muted text-sm">{body}</p>
            <button
                type="button"
                class="btn"
                on:click=move |_| on_clear.run(())
            >"Clear filters"</button>
        </div>
    }
}
