use leptos::prelude::*;

use super::item::{LauncherAction, LauncherEntry, LauncherGroup, LauncherIcon};
use crate::ui::components::shell::nav_catalog::SECTIONS;
use crate::ui::components::shell::theme_toggle::toggle_theme;
use crate::ui::state::activity::{set_tray_open, use_activity_state};
use crate::ui::state::sidebar::use_sidebar_state;

pub fn build_commands() -> Vec<LauncherEntry> {
    let activity = use_activity_state();
    let sidebar = use_sidebar_state();

    let mut entries = vec![
        LauncherEntry::new(
            LauncherIcon::Command,
            "Toggle light / dark theme",
            LauncherGroup::Commands,
            LauncherAction::Run(Callback::new(move |()| toggle_theme())),
        )
        .meta("Appearance"),
        LauncherEntry::new(
            LauncherIcon::Command,
            "Toggle activity tray",
            LauncherGroup::Commands,
            LauncherAction::Run(Callback::new(move |()| {
                set_tray_open(!activity.open.get_untracked());
            })),
        )
        .meta("Workspace")
        .keys("Ctrl `"),
        LauncherEntry::new(
            LauncherIcon::Command,
            "Toggle sidebar",
            LauncherGroup::Commands,
            LauncherAction::Run(Callback::new(move |()| sidebar.toggle())),
        )
        .meta("Workspace"),
    ];

    for section in SECTIONS {
        let section_label = section.label.unwrap_or("");
        for item in section.items {
            entries.push(
                LauncherEntry::new(
                    LauncherIcon::Nav(item.icon),
                    format!("Go to {}", item.label),
                    LauncherGroup::Commands,
                    LauncherAction::Navigate(item.href.to_string()),
                )
                .meta(section_label),
            );
        }
    }

    entries
}
