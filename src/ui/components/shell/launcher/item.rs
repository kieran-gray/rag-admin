use leptos::prelude::*;

use crate::ui::components::primitives::Status;
use crate::ui::components::shell::nav_catalog::{nav_icon, NavIcon};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LauncherGroup {
    Recent,
    Pages,
    Documents,
    Datasets,
    Runs,
    Connectors,
    Commands,
}

impl LauncherGroup {
    pub fn label(self) -> &'static str {
        match self {
            LauncherGroup::Recent => "Recent",
            LauncherGroup::Pages => "Pages",
            LauncherGroup::Documents => "Documents",
            LauncherGroup::Datasets => "Datasets",
            LauncherGroup::Runs => "Runs",
            LauncherGroup::Connectors => "Connectors",
            LauncherGroup::Commands => "Commands",
        }
    }

    pub fn order(self) -> u8 {
        match self {
            LauncherGroup::Recent => 0,
            LauncherGroup::Pages => 1,
            LauncherGroup::Documents => 2,
            LauncherGroup::Datasets => 3,
            LauncherGroup::Runs => 4,
            LauncherGroup::Connectors => 5,
            LauncherGroup::Commands => 6,
        }
    }
}

#[derive(Clone, Copy)]
pub enum LauncherIcon {
    Nav(NavIcon),
    Command,
    Recent,
}

#[derive(Clone)]
pub enum LauncherAction {
    Navigate(String),
    Run(Callback<()>),
}

#[derive(Clone)]
pub struct LauncherEntry {
    pub icon: LauncherIcon,
    pub label: String,
    pub meta: String,
    pub keys: Option<String>,
    pub status: Option<(Status, String)>,
    pub action: LauncherAction,
    pub group: LauncherGroup,
    pub haystack: String,
}

impl LauncherEntry {
    pub fn new(
        icon: LauncherIcon,
        label: impl Into<String>,
        group: LauncherGroup,
        action: LauncherAction,
    ) -> Self {
        let label = label.into();
        let haystack = label.to_lowercase();
        Self {
            icon,
            label,
            meta: String::new(),
            keys: None,
            status: None,
            action,
            group,
            haystack,
        }
    }

    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        let meta = meta.into();
        if !meta.is_empty() {
            self.haystack.push(' ');
            self.haystack.push_str(&meta.to_lowercase());
        }
        self.meta = meta;
        self
    }

    pub fn keys(mut self, keys: impl Into<String>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    pub fn status(mut self, kind: Status, label: impl Into<String>) -> Self {
        self.status = Some((kind, label.into()));
        self
    }
}

pub fn render_icon(icon: LauncherIcon) -> AnyView {
    match icon {
        LauncherIcon::Nav(n) => nav_icon(n),
        LauncherIcon::Command => view! {
            <span class="launcher-glyph" aria-hidden="true">"›"</span>
        }
        .into_any(),
        LauncherIcon::Recent => view! {
            <svg
                class="nav-icon-svg"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.7"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <circle cx="12" cy="12" r="9" />
                <path d="M12 7v5l3 2" />
            </svg>
        }
        .into_any(),
    }
}
