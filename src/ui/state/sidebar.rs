use leptos::prelude::*;

#[cfg(feature = "hydrate")]
const LS_COLLAPSED_KEY: &str = "search-crucible.sidebar.collapsed";

#[derive(Clone, Copy)]
pub struct SidebarState {
    pub collapsed: RwSignal<bool>,
}

impl SidebarState {
    pub fn toggle(&self) {
        self.collapsed.update(|c| *c = !*c);
    }
}

pub fn provide_sidebar_state() {
    let collapsed = RwSignal::new(false);
    provide_context(SidebarState { collapsed });
    restore_persisted_collapsed(collapsed);
    persist_collapsed_on_change(collapsed);
}

#[expect(
    clippy::expect_used,
    reason = "SidebarState is provided unconditionally by AppShell at startup"
)]
pub fn use_sidebar_state() -> SidebarState {
    use_context::<SidebarState>().expect("SidebarState context must be provided in AppShell")
}

#[cfg(feature = "hydrate")]
fn read_stored_collapsed() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|store| store.get_item(LS_COLLAPSED_KEY).ok().flatten())
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[cfg(feature = "hydrate")]
fn restore_persisted_collapsed(collapsed: RwSignal<bool>) {
    Effect::new(move |prev: Option<()>| {
        if prev.is_none() {
            collapsed.set(read_stored_collapsed());
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn restore_persisted_collapsed(_collapsed: RwSignal<bool>) {}

#[cfg(feature = "hydrate")]
fn persist_collapsed_on_change(collapsed: RwSignal<bool>) {
    Effect::new(move |prev: Option<()>| {
        let value = collapsed.get();
        if prev.is_none() {
            return;
        }
        if let Some(store) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            _ = store.set_item(LS_COLLAPSED_KEY, if value { "1" } else { "0" });
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn persist_collapsed_on_change(_collapsed: RwSignal<bool>) {}
