use leptos::prelude::*;

#[component]
pub fn Surface(
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional)] actions: Option<Children>,
    #[prop(optional)] raised: bool,
    #[prop(optional)] flush: bool,
    #[prop(optional, into)] class: Option<String>,
    children: Children,
) -> impl IntoView {
    let base = if raised { "surface-raised" } else { "surface" };
    let padding = if flush { "" } else { "p-4" };
    let extra = class.unwrap_or_default();
    let class = format!("{base} {padding} {extra}");

    let header = match (title.as_ref(), actions.is_some()) {
        (None, false) => None,
        _ => Some(view! {
            <div class="flex items-center justify-between mb-3 gap-3">
                <div class="section-title">{title.clone().unwrap_or_default()}</div>
                <div class="flex items-center gap-2">
                    {actions.map(|c| c())}
                </div>
            </div>
        }),
    };

    view! {
        <section class=class>
            {header}
            {children()}
        </section>
    }
}
