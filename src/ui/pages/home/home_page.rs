use leptos::prelude::*;

use crate::ui::components::primitives::PageHeader;

use super::recent::RecentPanel;
use super::setup::SetupPanel;
use super::start::StartPanel;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div>
            <PageHeader
                title="SearchCrucible"
                subtitle="Populate your vector store, then prove its retrieval quality.".to_string()
            />
            <div class="grid gap-6 lg:grid-cols-2">
                <StartPanel />
                <RecentPanel />
            </div>
            <div class="mt-6">
                <SetupPanel />
            </div>
        </div>
    }
}
