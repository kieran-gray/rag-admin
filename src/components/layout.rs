use leptos::prelude::*;
use leptos_router::components::A;
#[component]
pub fn Layout(children: Children) -> impl IntoView {
    view! {
        <div class="min-h-screen flex flex-col relative bg-[var(--color-page-bg)]">
            <div class="hidden xl:block">
                <div class="framing-line-v left-1/2 -ml-[576px]"></div>
                <div class="framing-line-v left-1/2 ml-[576px]"></div>
                <div class="framing-line-v" style="left: calc(50% - 576px - (50vw - 576px) / 3)"></div>
                <div class="framing-line-v" style="left: calc(50% + 576px + (50vw - 576px) / 3)"></div>
            </div>

            <header class="flex items-center justify-between z-10 relative">
                <div class="w-full relative">
                    <div class="max-w-6xl mx-auto flex items-center gap-8 px-6 py-4">
                        <div class="flex flex-col">
                            <div class="cyber-title cursor-default">
                                <span class="cyber-title-text" attr:data-text="RAG_ADMIN">
                                    "RAG_ADMIN"
                                </span>
                            </div>
                        </div>
                        <nav class="flex gap-6 text-sm font-medium pt-3 flex-1">
                            <A href="/" attr:class="hover:text-[var(--color-accent)] transition-colors">
                                <span class="opacity-50 mr-1">"01"</span>"DOCUMENTS"
                            </A>
                            <A href="/embed" attr:class="hover:text-[var(--color-accent)] transition-colors">
                                <span class="opacity-50 mr-1">"02"</span>"EMBED_TEST"
                            </A>
                            <A href="/new-settings" attr:class="hover:text-[var(--color-accent)] transition-colors">
                                <span class="opacity-50 mr-1">"03"</span>"PIPELINE_CONFIG"
                            </A>
                            <A href="/settings" attr:class="hover:text-[var(--color-accent)] transition-colors ml-auto">
                                <span class="opacity-50 mr-1">"04"</span>"LEGACY_SETTINGS"
                            </A>
                        </nav>
                    </div>
                    <div class="absolute bottom-0 left-0 right-0 framing-line-h"></div>
                </div>
            </header>

            <main class="flex-1 max-w-6xl mx-auto w-full relative">
                <div class="relative z-10 py-8">{children()}</div>
            </main>
        </div>
    }
}
