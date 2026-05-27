use leptos::prelude::*;

use super::activity_tray::ActivityTray;
use super::nav::AppNav;
use crate::ui::state::activity::{clamp_tray_height, provide_activity_state, use_activity_state};

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    provide_activity_state();
    let state = use_activity_state();
    let open = state.open;
    let height = state.height;

    let main_style = move || {
        if open.get() {
            format!("padding-bottom: {}px;", clamp_tray_height(height.get()))
        } else {
            String::new()
        }
    };

    view! {
        <div class="min-h-screen flex flex-col bg-[var(--color-page-bg)] mt-2">
            <AppNav />
            <main class="flex-1 w-full" style=main_style>
                <div class="max-w-6xl mx-auto px-6 py-8">{children()}</div>
            </main>
            <ActivityTray />
        </div>
    }
}
