use leptos::prelude::*;

use crate::shared::contracts::{
    ChunkingConfigurationCommandDto, ChunkingConfigurationDto, DeleteChunkingConfigurationDto,
};
use crate::ui::components::primitives::{Dialog, EmptyState, Surface};
use crate::ui::pages::configuration::commands::run_chunking_configuration_command;

#[component]
pub(super) fn ChunkingList(
    configurations: Vec<ChunkingConfigurationDto>,
    on_edit: Callback<ChunkingConfigurationDto>,
    on_delete: Callback<ChunkingConfigurationDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    if configurations.is_empty() {
        return view! {
            <Surface>
                <EmptyState
                    title="No chunking configurations yet"
                    body="A chunking configuration bundles a strategy (section, bert, llm) with its tunables. Create one and reuse it across ingestions and evaluation sweeps.".to_string()
                />
            </Surface>
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            {configurations.into_iter().map(|cc| view! {
                <ChunkingCard cc=cc on_edit=on_edit on_delete=on_delete busy=busy />
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn ChunkingCard(
    cc: ChunkingConfigurationDto,
    on_edit: Callback<ChunkingConfigurationDto>,
    on_delete: Callback<ChunkingConfigurationDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let cc_edit = cc.clone();
    let cc_delete = cc.clone();
    let name = cc.name.clone();
    let strategy_id = cc.config.strategy().as_str();
    let descriptor = cc.config.describe();
    let is_default = cc.is_default;

    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <h3 class="section-title flex items-center gap-2">
                    {name}
                    {is_default.then(|| view! {
                        <span class="pill pill-accent text-xs">"default"</span>
                    })}
                </h3>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{format!("strategy · {strategy_id}")}</span>
                    <span class="pill pill-neutral">{descriptor}</span>
                </div>
            </div>
            <div class="flex gap-2 shrink-0">
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_edit.run(cc_edit.clone())
                >
                    "Edit"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_delete.run(cc_delete.clone())
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}

#[component]
pub(super) fn DeleteConfirmDialog(
    target: ReadSignal<Option<ChunkingConfigurationDto>>,
    set_target: WriteSignal<Option<ChunkingConfigurationDto>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(cc) = target.get_untracked() else {
            return;
        };
        run_chunking_configuration_command(
            ChunkingConfigurationCommandDto::DeleteChunkingConfiguration(
                DeleteChunkingConfigurationDto {
                    chunking_configuration_id: cc.chunking_configuration_id,
                },
            ),
            "Chunking configuration deleted",
            set_busy,
            set_status,
            None,
            set_refresh,
            move || set_target.set(None),
        );
    };

    view! {
        <Dialog
            open=Signal::derive(move || target.get().is_some())
            title="Delete chunking configuration".to_string()
            subtitle="Existing indexings and runs already reference this by name. They keep working; new ones won't be able to pick it.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised p-3 rounded">
                    <span class="muted text-sm">"Configuration"</span>
                    <div class="text-text">{move || target.get().map(|cc| cc.name).unwrap_or_default()}</div>
                </div>
                <div class="flex justify-end gap-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="button" class="btn btn-primary" disabled=busy on:click=confirm>
                        {move || if busy.get() { "Deleting…" } else { "Delete" }}
                    </button>
                </div>
            </div>
        </Dialog>
    }
}
