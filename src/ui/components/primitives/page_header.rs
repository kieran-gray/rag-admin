use leptos::prelude::*;

#[component]
pub fn PageHeader(
    #[prop(into)] title: String,
    #[prop(optional, into)] subtitle: Option<String>,
    #[prop(optional)] actions: Option<Children>,
) -> impl IntoView {
    view! {
        <header class="flex flex-col items-start gap-3 mb-6 lg:flex-row lg:items-end lg:justify-between lg:gap-4">
            <div class="flex flex-col gap-1 min-w-0">
                <h1 class="page-title truncate">{title}</h1>
                {subtitle.map(|s| view! { <p class="muted text-sm">{s}</p> })}
            </div>
            {actions.map(|a| view! {
                <div class="flex items-center gap-2 flex-wrap lg:shrink-0">{a()}</div>
            })}
        </header>
    }
}
