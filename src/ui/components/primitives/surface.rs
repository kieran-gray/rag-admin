use leptos::prelude::*;

#[component]
pub fn Surface(
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional)] actions: Option<Children>,
    #[prop(optional)] raised: bool,
    #[prop(optional)] flush: bool,
    #[prop(optional)] sticky_header: bool,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let base = if raised { "surface-raised" } else { "surface" };
    let padding = if flush { "" } else { "p-4" };
    let extra = class.unwrap_or_default();
    let class = format!("{base} {padding} {extra}");

    let header_class = if sticky_header {
        "surface-header surface-header-sticky"
    } else {
        "surface-header"
    };
    let header = if title.is_some() || actions.is_some() {
        Some(view! {
            <div class=header_class>
                <div class="section-title">{title.unwrap_or_default()}</div>
                <div class="flex items-center gap-2">
                    {actions.map(|c| c())}
                </div>
            </div>
        })
    } else {
        None
    };

    view! {
        <section class=class>
            {header}
            {children()}
        </section>
    }
}
