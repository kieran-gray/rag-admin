use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::EvaluationResultSplit;

use super::question_drilldown::QuestionDrilldown;

#[derive(Clone, PartialEq, Eq)]
pub struct DrawerTarget {
    pub row_key: String,
    pub variant_label: String,
    pub split: EvaluationResultSplit,
}

#[component]
pub fn DrilldownDrawer(
    run_id: Uuid,
    target: ReadSignal<Option<DrawerTarget>>,
    on_close: Callback<()>,
) -> impl IntoView {
    let open = move || target.with(Option::is_some);
    view! {
        <Show when=open>
            {move || target.get().map(|t| {
                let title = t.variant_label.clone();
                let split = t.split;
                let label = t.variant_label.clone();
                view! {
                    <div
                        class="drilldown-drawer-overlay"
                        on:click=move |ev| {
                            let target_el = ev.target();
                            let current = ev.current_target();
                            if target_el == current {
                                on_close.run(());
                            }
                        }
                    >
                        <aside class="drilldown-drawer" role="dialog" aria-label="Per-question diagnostics">
                            <header class="drilldown-drawer-head">
                                <div class="min-w-0">
                                    <div class="eyebrow">{"Per-question · ".to_string() + split.as_str()}</div>
                                    <div class="font-mono text-sm truncate">{title}</div>
                                </div>
                                <button
                                    type="button"
                                    class="btn btn-ghost btn-compact"
                                    aria-label="Close drawer"
                                    on:click=move |_| on_close.run(())
                                >
                                    "✕"
                                </button>
                            </header>
                            <div class="drilldown-drawer-body">
                                <QuestionDrilldown
                                    run_id=run_id
                                    variant_label=label
                                    split=split
                                />
                            </div>
                        </aside>
                    </div>
                }
            })}
        </Show>
    }
}
