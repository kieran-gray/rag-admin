use leptos::prelude::*;
use leptos_router::components::A;

struct StartLink {
    href: &'static str,
    label: &'static str,
    hint: &'static str,
    glyph: &'static str,
}

const LINKS: &[StartLink] = &[
    StartLink {
        href: "/documents",
        label: "Import documents",
        hint: "bring your corpus in",
        glyph: "＋",
    },
    StartLink {
        href: "/pipeline/profiles",
        label: "New index profile",
        hint: "embedding + vector index",
        glyph: "▦",
    },
    StartLink {
        href: "/workflows/evaluate",
        label: "Run an evaluation",
        hint: "prove retrieval quality",
        glyph: "✓",
    },
    StartLink {
        href: "/playground/retrieve",
        label: "Open playground",
        hint: "interactive top-K search",
        glyph: "▷",
    },
    StartLink {
        href: "/evaluation/runs",
        label: "Browse runs",
        hint: "past evaluations",
        glyph: "≣",
    },
];

#[component]
pub(super) fn StartPanel() -> impl IntoView {
    view! {
        <section class="surface p-4 space-y-3">
            <h2 class="section-title">"Start"</h2>
            <ul class="flex flex-col gap-1">
                {LINKS.iter().map(|l| view! {
                    <li>
                        <A
                            href=l.href
                            attr:class="flex items-center gap-3 rounded px-2 py-2 hover:bg-[var(--color-surface-hover)]"
                        >
                            <span class="faint w-5 text-center" aria-hidden="true">{l.glyph}</span>
                            <span class="min-w-0">
                                <span class="block text-sm text-text">{l.label}</span>
                                <span class="block text-xs muted">{l.hint}</span>
                            </span>
                        </A>
                    </li>
                }).collect_view()}
            </ul>
        </section>
    }
}
