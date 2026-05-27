use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::EvaluationResultSplit;

use super::question_drilldown::QuestionDrilldown;

#[derive(Clone, PartialEq, Eq)]
pub struct ModalTarget {
    pub row_key: String,
    pub variant_label: String,
    pub split: EvaluationResultSplit,
}

#[component]
pub fn DrilldownModal(
    run_id: Uuid,
    target: ReadSignal<Option<ModalTarget>>,
    on_close: Callback<()>,
) -> impl IntoView {
    let open = move || target.with(Option::is_some);

    #[cfg(feature = "hydrate")]
    let esc_listener = StoredValue::new_local(None::<self::hydrate::EscListener>);

    #[cfg(feature = "hydrate")]
    Effect::new(move |_| {
        if open() {
            esc_listener.set_value(Some(self::hydrate::install_esc(on_close)));
        } else {
            esc_listener.set_value(None);
        }
    });

    view! {
        <Show when=open>
            {move || target.get().map(|t| {
                let title = t.variant_label.clone();
                let split = t.split;
                let label = t.variant_label.clone();
                view! {
                    <div
                        class="drilldown-modal-overlay"
                        on:click=move |ev| {
                            let target_el = ev.target();
                            let current = ev.current_target();
                            if target_el == current {
                                on_close.run(());
                            }
                        }
                    >
                        <div class="drilldown-modal" role="dialog" aria-modal="true" aria-label="Per-question diagnostics">
                            <header class="drilldown-modal-head">
                                <div class="min-w-0">
                                    <div class="eyebrow">{"Per-question · ".to_string() + split.as_str()}</div>
                                    <div class="font-mono text-sm truncate">{title}</div>
                                </div>
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-compact"
                                    aria-label="Close"
                                    on:click=move |_| on_close.run(())
                                >
                                    "✕"
                                </button>
                            </header>
                            <div class="drilldown-modal-body">
                                <QuestionDrilldown
                                    run_id=run_id
                                    variant_label=label
                                    split=split
                                />
                            </div>
                        </div>
                    </div>
                }
            })}
        </Show>
    }
}

#[cfg(feature = "hydrate")]
mod hydrate {
    use leptos::prelude::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    pub struct EscListener {
        closure: Option<Closure<dyn FnMut(web_sys::KeyboardEvent)>>,
    }

    impl Drop for EscListener {
        fn drop(&mut self) {
            if let (Some(window), Some(c)) = (web_sys::window(), self.closure.take()) {
                _ = window
                    .remove_event_listener_with_callback("keydown", c.as_ref().unchecked_ref());
            }
        }
    }

    pub fn install_esc(on_close: Callback<()>) -> EscListener {
        let Some(window) = web_sys::window() else {
            return EscListener { closure: None };
        };
        let closure =
            Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
                if ev.key() == "Escape" {
                    on_close.run(());
                }
            });
        _ = window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
        EscListener {
            closure: Some(closure),
        }
    }
}
