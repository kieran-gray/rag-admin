use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct LauncherState {
    pub open: RwSignal<bool>,
}

impl LauncherState {
    pub fn open(&self) {
        self.open.set(true);
    }

    pub fn close(&self) {
        self.open.set(false);
    }

    pub fn toggle(&self) {
        self.open.update(|o| *o = !*o);
    }
}

pub fn provide_launcher_state() {
    provide_context(LauncherState {
        open: RwSignal::new(false),
    });
}

#[expect(
    clippy::expect_used,
    reason = "LauncherState is provided unconditionally by AppShell at startup"
)]
pub fn use_launcher_state() -> LauncherState {
    use_context::<LauncherState>().expect("LauncherState context must be provided in AppShell")
}
