use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use crate::server_functions::docs::{get_guide, list_guides};
use crate::shared::contracts::GuideSectionDto;
use crate::ui::components::markdown_body::MarkdownBody;
use crate::ui::components::primitives::{EmptyState, PageHeader};

#[component]
pub fn DocsPage() -> impl IntoView {
    let params = use_params_map();
    let slug = Memo::new(move |_| params.with(|p| p.get("slug").map(|s| s.to_string())));

    let sections = Resource::new(
        || (),
        |()| async move { list_guides().await.map_err(|e| e.to_string()) },
    );

    let guide = Resource::new(
        move || slug.get(),
        |slug| async move {
            match slug {
                Some(slug) => get_guide(slug).await.map_err(|e| e.to_string()),
                None => Ok(None),
            }
        },
    );

    view! {
        <PageHeader
            title="Documentation"
            subtitle="Guides for adding documents, running evaluations, and tuning retrieval."
        />
        <div class="docs-layout">
            <aside class="docs-sidebar">
                <Transition fallback=|| view! { <p class="text-xs muted">"Loading…"</p> }>
                    {move || sections.get().map(|res| match res {
                        Err(e) => view! {
                            <div class="log-line-error text-sm">{format!("Failed to load guides: {e}")}</div>
                        }.into_any(),
                        Ok(sections) => view! { <DocsNav sections=sections active=slug /> }.into_any(),
                    })}
                </Transition>
            </aside>
            <div class="docs-content">
                <Transition fallback=|| view! { <p class="text-xs muted">"Loading…"</p> }>
                    {move || {
                        let has_slug = slug.get().is_some();
                        guide.get().map(|res| match res {
                            Err(e) => view! {
                                <div class="log-line-error text-sm">{format!("Failed to load guide: {e}")}</div>
                            }.into_any(),
                            Ok(Some(g)) => view! {
                                <article class="surface p-6 docs-article">
                                    <MarkdownBody blocks=g.blocks />
                                </article>
                            }.into_any(),
                            Ok(None) if has_slug => view! {
                                <EmptyState
                                    title="Guide not found"
                                    body="That guide doesn't exist. Pick one from the list on the left."
                                />
                            }.into_any(),
                            Ok(None) => view! {
                                <div class="surface p-6">
                                    <p class="section-title">"Welcome"</p>
                                    <p class="muted text-sm max-w-prose">
                                        "Choose a guide from the list to get started."
                                    </p>
                                </div>
                            }.into_any(),
                        })
                    }}
                </Transition>
            </div>
        </div>
    }
}

#[component]
fn DocsNav(sections: Vec<GuideSectionDto>, active: Memo<Option<String>>) -> impl IntoView {
    view! {
        <nav class="docs-nav">
            {sections.into_iter().map(|section| {
                let label = section.label;
                view! {
                    <div class="docs-nav-section">
                        <span class="eyebrow">{label}</span>
                        <ul class="docs-nav-list">
                            {section.guides.into_iter().map(|g| {
                                let slug = g.slug;
                                let href = format!("/docs/{slug}");
                                let is_active = move || active.get().as_deref() == Some(slug.as_str());
                                view! {
                                    <li>
                                        <A
                                            href=href
                                            attr:class=move || if is_active() {
                                                "docs-nav-link is-active"
                                            } else {
                                                "docs-nav-link"
                                            }
                                        >
                                            {g.title}
                                        </A>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    </div>
                }
            }).collect_view()}
        </nav>
    }
}
