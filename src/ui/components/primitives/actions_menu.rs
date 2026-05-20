use leptos::prelude::*;

#[derive(Clone)]
pub struct ActionItem {
    pub label: String,
    pub danger: bool,
    pub on_select: Callback<()>,
}

#[component]
pub fn ActionsMenu(
    #[prop(into, default = "Actions".to_string())] label: String,
    items: Vec<ActionItem>,
) -> impl IntoView {
    let (open, set_open) = signal(false);

    #[cfg(feature = "hydrate")]
    let outside_listener = StoredValue::new_local(None::<self::hydrate::OutsideClick>);

    let toggle = move |_| {
        let next = !open.get_untracked();
        set_open.set(next);
        #[cfg(feature = "hydrate")]
        {
            if next {
                outside_listener.set_value(Some(self::hydrate::watch_outside_clicks(
                    ".actions-menu",
                    set_open,
                )));
            } else {
                outside_listener.set_value(None);
            }
        }
    };

    let close = move || {
        set_open.set(false);
        #[cfg(feature = "hydrate")]
        outside_listener.set_value(None);
    };

    let items = StoredValue::new(items);

    view! {
        <div class="actions-menu">
            <button
                type="button"
                class="btn actions-menu-trigger"
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=toggle
            >
                <span>{label}</span>
                <span class="actions-menu-caret" aria-hidden="true">"▾"</span>
            </button>
            {move || open.get().then(|| {
                let rendered = items.with_value(Clone::clone);
                view! {
                    <div class="actions-menu-popover" role="menu">
                        {rendered.into_iter().map(|item| {
                            let on_select = item.on_select;
                            let row_class = if item.danger {
                                "actions-menu-item is-danger"
                            } else {
                                "actions-menu-item"
                            };
                            view! {
                                <button
                                    type="button"
                                    class=row_class
                                    role="menuitem"
                                    on:click=move |_| {
                                        close();
                                        on_select.run(());
                                    }
                                >
                                    {item.label}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                }
            })}
        </div>
    }
}

#[cfg(feature = "hydrate")]
mod hydrate {
    use leptos::prelude::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    pub struct OutsideClick {
        closure: Option<Closure<dyn FnMut(web_sys::MouseEvent)>>,
    }

    impl Drop for OutsideClick {
        fn drop(&mut self) {
            if let (Some(window), Some(c)) = (web_sys::window(), self.closure.take()) {
                _ = window
                    .remove_event_listener_with_callback("mousedown", c.as_ref().unchecked_ref());
            }
        }
    }

    pub fn watch_outside_clicks(scope: &str, set_open: WriteSignal<bool>) -> OutsideClick {
        let Some(window) = web_sys::window() else {
            return OutsideClick { closure: None };
        };
        let scope = scope.to_string();
        let closure =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |ev: web_sys::MouseEvent| {
                let target = ev
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
                if let Some(el) = target {
                    if el.closest(&scope).ok().flatten().is_some() {
                        return;
                    }
                }
                set_open.set(false);
            });
        _ = window.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref());
        OutsideClick {
            closure: Some(closure),
        }
    }
}
