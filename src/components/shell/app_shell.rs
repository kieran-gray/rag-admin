use leptos::prelude::*;

use super::activity_drawer::ActivityDrawer;
use super::nav::AppNav;
use crate::components::activity::provide_activity_state;

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    provide_activity_state();

    view! {
        <div class="min-h-screen flex flex-col bg-[var(--color-page-bg)]">
            <AppNav />
            <main class="flex-1 w-full">
                <div class="max-w-6xl mx-auto px-6 py-8">{children()}</div>
            </main>
            <ActivityDrawer />
        </div>
    }
}
