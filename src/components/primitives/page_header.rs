use leptos::prelude::*;

#[component]
pub fn PageHeader(
    #[prop(into)] title: String,
    #[prop(optional, into)] eyebrow: Option<String>,
    #[prop(optional, into)] subtitle: Option<String>,
    #[prop(optional)] actions: Option<Children>,
) -> impl IntoView {
    view! {
        <header class="flex items-end justify-between gap-4 mb-6">
            <div class="flex flex-col gap-1 min-w-0">
                {eyebrow.map(|e| view! { <div class="eyebrow">{e}</div> })}
                <h1 class="page-title truncate">{title}</h1>
                {subtitle.map(|s| view! { <p class="muted text-sm">{s}</p> })}
            </div>
            {actions.map(|a| view! {
                <div class="flex items-center gap-2 shrink-0">{a()}</div>
            })}
        </header>
    }
}
