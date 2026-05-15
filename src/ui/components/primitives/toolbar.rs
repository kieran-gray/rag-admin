use leptos::prelude::*;

#[component]
pub fn Toolbar(children: Children) -> impl IntoView {
    view! { <div class="toolbar mb-4">{children()}</div> }
}
