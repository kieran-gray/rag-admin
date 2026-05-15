use leptos::prelude::*;

#[component]
pub fn Dialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] title: String,
    #[prop(optional, into)] subtitle: Option<String>,
    on_close: Callback<()>,
    children: ChildrenFn,
) -> impl IntoView {
    let children = StoredValue::new(children);
    view! {
        <Show when=move || open.get()>
            <div
                class="fixed inset-0 z-50 flex items-start justify-center pt-20 bg-black/60 backdrop-blur-sm"
                on:click=move |ev| {

                    let target = ev.target();
                    let current = ev.current_target();
                    if target == current {
                        on_close.run(());
                    }
                }
            >
                <div class="surface w-full max-w-lg mx-4 max-h-[80vh] flex flex-col overflow-hidden">
                    <div class="flex items-start justify-between gap-3 px-5 py-4 border-b border-[var(--color-border)]">
                        <div class="min-w-0">
                            <h2 class="section-title">{title.clone()}</h2>
                            {subtitle.clone().map(|s| view! { <p class="muted text-sm mt-1">{s}</p> })}
                        </div>
                        <button
                            type="button"
                            class="btn-ghost btn"
                            aria-label="Close"
                            on:click=move |_| on_close.run(())
                        >
                            "✕"
                        </button>
                    </div>
                    <div class="p-5 overflow-y-auto">
                        {children.with_value(|c| c())}
                    </div>
                </div>
            </div>
        </Show>
    }
}
