use leptos::prelude::*;
use leptos_router::hooks::use_location;

use super::activity_tray::ActivityTray;
use super::breadcrumb::Breadcrumb;
use super::launcher::CommandLauncher;
use super::nav::AppSidebar;
use crate::ui::state::activity::{clamp_tray_height, provide_activity_state, use_activity_state};
use crate::ui::state::launcher::provide_launcher_state;
use crate::ui::state::recent::record_location;
use crate::ui::state::sidebar::{provide_sidebar_state, use_sidebar_state};

#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    provide_activity_state();
    provide_sidebar_state();
    provide_launcher_state();
    let state = use_activity_state();
    let open = state.open;
    let height = state.height;

    let collapsed = use_sidebar_state().collapsed;

    let location = use_location();
    Effect::new(move |_| {
        record_location(&location.pathname.get());
    });

    let main_style = move || {
        if open.get() {
            format!("padding-bottom: {}px;", clamp_tray_height(height.get()))
        } else {
            String::new()
        }
    };

    let layout_class = move || {
        if collapsed.get() {
            "app-layout is-collapsed bg-[var(--color-page-bg)]"
        } else {
            "app-layout bg-[var(--color-page-bg)]"
        }
    };

    view! {
        <div class=layout_class>
            <AppSidebar />
            <main class="app-main" style=main_style>
                <Breadcrumb />
                <div class="max-w-6xl mx-auto px-8 py-8">{children()}</div>
            </main>
            <ActivityTray />
            <CommandLauncher />
        </div>
    }
}
