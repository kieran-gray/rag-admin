use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

#[component]
pub fn QueryInput(
    value: RwSignal<String>,
    busy: Signal<bool>,
    on_submit: Callback<()>,
    #[prop(into)] placeholder: String,
    #[prop(optional, into)] submit_label: Option<String>,
    #[prop(optional, into)] busy_label: Option<String>,
    #[prop(optional)] can_submit: Option<Signal<bool>>,
    #[prop(optional)] on_stop: Option<Callback<()>>,
) -> impl IntoView {
    let submit_label = submit_label.unwrap_or_else(|| "Run".to_string());
    let busy_label = busy_label.unwrap_or_else(|| "Running…".to_string());

    let can_submit_signal: Signal<bool> = can_submit.unwrap_or_else(|| {
        Signal::derive(move || !busy.get() && !value.with(|v| v.trim().is_empty()))
    });

    let trigger_submit = move || {
        if can_submit_signal.get_untracked() {
            on_submit.run(());
        }
    };

    let on_keydown = move |ev: KeyboardEvent| {
        if ev.key() == "Enter" && (ev.meta_key() || ev.ctrl_key()) {
            ev.prevent_default();
            trigger_submit();
        }
    };

    let submit_label_for_button = submit_label.clone();
    let busy_label_for_button = busy_label.clone();
    let stop_available = on_stop.is_some();

    let primary_class = move || {
        if busy.get() && stop_available {
            "btn btn-stop"
        } else {
            "btn btn-primary"
        }
    };
    let primary_disabled = move || {
        if busy.get() {
            !stop_available
        } else {
            !can_submit_signal.get()
        }
    };
    let primary_label = move || {
        if busy.get() {
            if stop_available {
                "Stop".to_string()
            } else {
                busy_label_for_button.clone()
            }
        } else {
            submit_label_for_button.clone()
        }
    };
    let primary_click = move |_| {
        if busy.get_untracked() {
            if let Some(cb) = on_stop {
                cb.run(());
            }
        } else {
            trigger_submit();
        }
    };

    view! {
        <div class="query-input">
            <textarea
                class="query-input-textarea"
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |ev| value.set(event_target_value(&ev))
                on:keydown=on_keydown
                rows="3"
            ></textarea>
            <div class="query-input-actions">
                <span class="query-input-hint faint">"⌘+↵ to send"</span>
                <button
                    type="button"
                    class=primary_class
                    disabled=primary_disabled
                    on:click=primary_click
                >
                    {primary_label}
                </button>
            </div>
        </div>
    }
}
