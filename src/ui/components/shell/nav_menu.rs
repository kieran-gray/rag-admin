use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

#[derive(Clone, Copy)]
pub struct NavMenuItem {
    pub href: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

#[component]
pub fn NavMenu(
    label: &'static str,
    prefix: &'static str,
    items: &'static [NavMenuItem],
) -> impl IntoView {
    let location = use_location();
    let (open, set_open) = signal(false);

    #[cfg(feature = "hydrate")]
    let outside_listener = StoredValue::new_local(None::<self::hydrate::OutsideClick>);

    let menu_id: String = format!("nav-menu-{}", label.to_ascii_lowercase());
    let close_class = format!(".{menu_id}");

    let toggle = {
        let close_class = close_class.clone();
        move |_| {
            let next = !open.get_untracked();
            set_open.set(next);
            #[cfg(feature = "hydrate")]
            {
                if next {
                    outside_listener.set_value(Some(self::hydrate::watch_outside_clicks(
                        &close_class,
                        set_open,
                    )));
                } else {
                    outside_listener.set_value(None);
                }
            }
            #[cfg(not(feature = "hydrate"))]
            let _ = &close_class;
        }
    };

    let close = move || {
        set_open.set(false);
        #[cfg(feature = "hydrate")]
        outside_listener.set_value(None);
    };

    let parent_active = move || {
        let path = location.pathname.get();
        path == prefix || path.starts_with(&format!("{prefix}/"))
    };

    let item_active = move |href: &'static str| {
        let path = location.pathname.get();
        path == href || path.starts_with(&format!("{href}/"))
    };

    let parent_class = move || {
        if parent_active() {
            "app-nav-link app-nav-menu-trigger is-active"
        } else {
            "app-nav-link app-nav-menu-trigger"
        }
    };

    view! {
        <div class=format!("app-nav-menu {menu_id}")>
            <button
                type="button"
                class=parent_class
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=toggle
            >
                <span>{label}</span>
                <span class="app-nav-menu-caret" aria-hidden="true">"▾"</span>
            </button>
            {move || open.get().then(|| {
                view! {
                    <div class="app-nav-menu-popover" role="menu">
                        {items.iter().map(|item| {
                            let href = item.href;
                            let active = move || item_active(href);
                            let row_class = move || if active() {
                                "app-nav-menu-item is-active"
                            } else {
                                "app-nav-menu-item"
                            };
                            view! {
                                <A
                                    href=href
                                    attr:class=row_class
                                    attr:role="menuitem"
                                    on:click=move |_| close()
                                >
                                    <span class="app-nav-menu-item-bar" aria-hidden="true"></span>
                                    <span class="app-nav-menu-item-text">
                                        <span class="app-nav-menu-item-label">{item.label}</span>
                                        <span class="app-nav-menu-item-hint">{item.hint}</span>
                                    </span>
                                </A>
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
