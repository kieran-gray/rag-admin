use leptos::prelude::*;

#[component]
pub fn EmptyState(
    #[prop(into)] title: String,
    #[prop(optional, into)] body: Option<String>,
    #[prop(optional)] action: Option<Children>,
) -> impl IntoView {
    view! {
        <div class="flex flex-col items-start gap-2 py-6">
            <div class="section-title">{title}</div>
            {body.map(|b| view! { <p class="muted text-sm max-w-prose">{b}</p> })}
            {action.map(|a| view! { <div class="mt-2">{a()}</div> })}
        </div>
    }
}
