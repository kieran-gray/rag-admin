use leptos::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineStatusMessage {
    pub ok: bool,
    pub text: String,
}

impl InlineStatusMessage {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            ok: true,
            text: text.into(),
        }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self {
            ok: false,
            text: text.into(),
        }
    }
}

#[component]
pub fn InlineStatus(#[prop(into)] status: Signal<Option<InlineStatusMessage>>) -> impl IntoView {
    view! {
        {move || status.get().map(|m| {
            let cls = if m.ok { "log-line-success" } else { "log-line-error" };
            view! { <div class=format!("{cls} text-sm")>{m.text}</div> }
        })}
    }
}
