use leptos::prelude::*;

#[cfg(feature = "hydrate")]
const STORAGE_KEY: &str = "search-crucible-theme";

#[component]
pub fn ThemeToggle() -> impl IntoView {
    let (is_light, set_is_light) = signal(false);

    #[cfg(feature = "hydrate")]
    {
        Effect::new(move |_| {
            let initial = current_dom_theme_is_light();
            set_is_light.set(initial);
        });
    }

    let toggle = move |_| {
        let next = !is_light.get_untracked();
        set_is_light.set(next);
        #[cfg(feature = "hydrate")]
        write_dom_theme(next);
    };

    let label = move || {
        if is_light.get() {
            "Switch to dark mode"
        } else {
            "Switch to light mode"
        }
    };

    view! {
        <button
            type="button"
            class="theme-toggle"
            aria-label=label
            title=label
            aria-pressed=move || if is_light.get() { "true" } else { "false" }
            on:click=toggle
        >
            <span class="theme-toggle-glyph" aria-hidden="true">
                {move || if is_light.get() {
                    sun_svg().into_any()
                } else {
                    moon_svg().into_any()
                }}
            </span>
        </button>
    }
}

fn moon_svg() -> impl IntoView {
    view! {
        <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M21 12.79A9 9 0 1 1 11.21 3a7 7 0 0 0 9.79 9.79Z" />
        </svg>
    }
}

fn sun_svg() -> impl IntoView {
    view! {
        <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2v2" />
            <path d="M12 20v2" />
            <path d="m4.93 4.93 1.41 1.41" />
            <path d="m17.66 17.66 1.41 1.41" />
            <path d="M2 12h2" />
            <path d="M20 12h2" />
            <path d="m4.93 19.07 1.41-1.41" />
            <path d="m17.66 6.34 1.41-1.41" />
        </svg>
    }
}

#[cfg(feature = "hydrate")]
fn current_dom_theme_is_light() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Some(root) = document.document_element() else {
        return false;
    };
    matches!(root.get_attribute("data-theme").as_deref(), Some("light"))
}

#[cfg(feature = "hydrate")]
fn write_dom_theme(is_light: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.document_element() else {
        return;
    };
    if is_light {
        _ = root.set_attribute("data-theme", "light");
    } else {
        _ = root.remove_attribute("data-theme");
    }
    if let Ok(Some(storage)) = window.local_storage() {
        let value = if is_light { "light" } else { "dark" };
        _ = storage.set_item(STORAGE_KEY, value);
    }
}

#[cfg(feature = "hydrate")]
pub fn toggle_theme() {
    write_dom_theme(!current_dom_theme_is_light());
}

#[cfg(not(feature = "hydrate"))]
pub fn toggle_theme() {}
