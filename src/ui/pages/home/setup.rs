use leptos::prelude::*;
use leptos_router::components::A;

use crate::server_functions::configuration::get_configuration;
use crate::server_functions::evaluation::get_recent_runs;
use crate::server_functions::source_document::list_documents;

const HIDE_KEY: &str = "rag-admin.setup.hidden";

#[component]
pub(super) fn SetupPanel() -> impl IntoView {
    let data = Resource::new(
        || (),
        |_| async move {
            let has_index_profile = get_configuration()
                .await
                .map(|c| !c.index_profiles.is_empty())
                .unwrap_or(false);
            let has_documents = list_documents()
                .await
                .unwrap_or_default()
                .iter()
                .any(|d| d.document_id.is_some());
            let has_runs = !get_recent_runs(1).await.unwrap_or_default().is_empty();
            (has_index_profile, has_documents, has_runs)
        },
    );

    let (hidden, set_hidden) = signal(false);
    Effect::new(move |_| set_hidden.set(read_hidden()));

    let hide = move |_| {
        write_hidden(true);
        set_hidden.set(true);
    };

    view! {
        <Transition fallback=|| view! {
            <section class="surface p-4 space-y-3">
                <h2 class="section-title">"Setup"</h2>
                <p class="text-xs muted py-1">"Checking setup…"</p>
            </section>
        }>
            {move || data.get().map(|(index_profile, documents, runs)| {
                let all_done = index_profile && documents && runs;
                if all_done && hidden.get() {
                    return ().into_any();
                }
                view! {
                    <section class="surface p-4 space-y-3">
                        <div class="flex items-center justify-between gap-2">
                            <h2 class="section-title">"Setup"</h2>
                            {all_done.then(|| view! {
                                <button type="button" class="btn btn-ghost btn-xs muted" on:click=hide>
                                    "Hide"
                                </button>
                            })}
                        </div>
                        <ul class="flex flex-col gap-1">
                            <SetupRow
                                done=index_profile
                                label="Set up an index profile"
                                hint="embedding + vector index"
                                href="/pipeline/profiles"
                            />
                            <SetupRow
                                done=documents
                                label="Import your first documents"
                                hint="bring a corpus in"
                                href="/documents"
                            />
                            <SetupRow
                                done=runs
                                label="Run your first evaluation"
                                hint="prove retrieval quality"
                                href="/workflows/evaluate"
                            />
                        </ul>
                    </section>
                }
                .into_any()
            })}
        </Transition>
    }
}

#[component]
fn SetupRow(
    done: bool,
    label: &'static str,
    hint: &'static str,
    href: &'static str,
) -> impl IntoView {
    view! {
        <li class="flex items-center gap-3 px-2 py-1.5">
            <span class=if done { "text-[var(--color-accent)]" } else { "faint" }>
                {if done { "☑" } else { "☐" }}
            </span>
            <span class="min-w-0 flex-1">
                <span class="block text-sm text-text">{label}</span>
                <span class="block text-xs muted">{hint}</span>
            </span>
            {(!done).then(|| view! {
                <A href=href attr:class="btn btn-ghost btn-xs shrink-0">"Start →"</A>
            })}
        </li>
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn read_hidden() -> bool {
    storage()
        .and_then(|s| s.get_item(HIDE_KEY).ok().flatten())
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn write_hidden(value: bool) {
    if let Some(store) = storage() {
        _ = store.set_item(HIDE_KEY, if value { "1" } else { "0" });
    }
}
