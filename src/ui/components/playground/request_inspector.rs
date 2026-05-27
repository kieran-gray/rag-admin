use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;

#[component]
pub fn RequestInspector(
    #[prop(into)] body: Signal<String>,
    #[prop(optional, into)] label: Option<String>,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    let (copied, set_copied) = signal(false);
    let label = label.unwrap_or_else(|| "Inspect request".to_string());

    let copy_to_clipboard = move |_| {
        let text = body.get_untracked();
        copy_text(&text);
        set_copied.set(true);
        leptos::task::spawn_local(async move {
            TimeoutFuture::new(1500).await;
            set_copied.set(false);
        });
    };

    view! {
        <div class="request-inspector">
            <button
                type="button"
                class="request-inspector-toggle"
                on:click=move |_| set_expanded.update(|v| *v = !*v)
            >
                <span class="request-inspector-caret" data-expanded=move || expanded.get().to_string()>"▸"</span>
                <span>{label}</span>
            </button>
            {move || expanded.get().then(|| view! {
                <div class="request-inspector-body">
                    <div class="request-inspector-toolbar">
                        <button
                            type="button"
                            class="btn btn-sm btn-ghost"
                            on:click=copy_to_clipboard
                        >
                            {move || if copied.get() { "Copied" } else { "Copy" }}
                        </button>
                    </div>
                    <pre class="request-inspector-pre">{move || body.get()}</pre>
                </div>
            })}
        </div>
    }
}

#[cfg(feature = "hydrate")]
fn copy_text(text: &str) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        drop(clipboard.write_text(text));
    }
}

#[cfg(not(feature = "hydrate"))]
fn copy_text(_text: &str) {}
