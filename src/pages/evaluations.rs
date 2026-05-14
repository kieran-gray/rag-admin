use leptos::prelude::*;

use crate::components::primitives::{EmptyState, PageHeader, Surface};

#[component]
pub fn EvaluationsPage() -> impl IntoView {
    view! {
        <div>
            <PageHeader
                title="Evaluations"
                subtitle="Cross-document run history and best-variant leaderboard.".to_string()
            />
            <Surface title="Best variants".to_string()>
                <EmptyState
                    title="No evaluation runs yet"
                    body="Open a document and run an evaluation to populate the leaderboard.".to_string()
                />
            </Surface>
        </div>
    }
}
