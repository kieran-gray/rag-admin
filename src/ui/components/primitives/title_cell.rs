use leptos::prelude::*;

#[component]
pub fn TitleCell(#[prop(into)] title: String, #[prop(into)] sub: String) -> impl IntoView {
    let sub_title = sub.clone();
    view! {
        <div class="list-page-title-cell">
            <div class="list-page-title-text">{title}</div>
            <div class="list-page-title-sub" title=sub_title>{sub}</div>
        </div>
    }
}
