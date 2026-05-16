use leptos::prelude::*;

use super::dialog::Dialog;

#[component]
pub fn ConfirmDialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] title: String,
    #[prop(into)] message: String,
    #[prop(into)] confirm_label: String,
    #[prop(into)] confirm_busy_label: String,
    #[prop(into)] busy: Signal<bool>,
    #[prop(default = false)] danger: bool,
    on_confirm: Callback<()>,
    on_close: Callback<()>,
) -> impl IntoView {
    let message = StoredValue::new(message);
    let confirm_label = StoredValue::new(confirm_label);
    let confirm_busy_label = StoredValue::new(confirm_busy_label);
    let confirm_class = if danger {
        "btn btn-danger"
    } else {
        "btn btn-primary"
    };

    view! {
        <Dialog open=open title=title on_close=on_close>
            <div class="space-y-4">
                <p class="text-text">{move || message.get_value()}</p>
                <div class="flex justify-end gap-2">
                    <button
                        type="button"
                        class="btn"
                        disabled=move || busy.get()
                        on:click=move |_| on_close.run(())
                    >
                        "Cancel"
                    </button>
                    <button
                        type="button"
                        class=confirm_class
                        disabled=move || busy.get()
                        on:click=move |_| on_confirm.run(())
                    >
                        {move || if busy.get() {
                            confirm_busy_label.get_value()
                        } else {
                            confirm_label.get_value()
                        }}
                    </button>
                </div>
            </div>
        </Dialog>
    }
}
