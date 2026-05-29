use leptos::prelude::*;

use crate::shared::contracts::{
    ChunkingConfigurationCommandDto, ChunkingConfigurationDto, DeleteChunkingConfigurationDto,
};
use crate::shared::reference_data::{ChunkStrategy, ChunkerDefinition};
use crate::ui::components::primitives::{Dialog, EmptyState, InlineStatusMessage, Surface};
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

    let mut groups: Vec<StrategyGroup> = ChunkStrategy::all()
        .iter()
        .map(StrategyGroup::empty)
        .collect();
    for cc in configurations {
        let strategy = cc.config.strategy();
        if let Some(group) = groups.iter_mut().find(|group| group.strategy == strategy) {
            group.members.push(cc);
        }
    }

    view! {
        <div class="chunking-groups">
            {groups
                .into_iter()
                .filter(|group| !group.members.is_empty())
                .map(|group| view! {
                    <section class="chunking-group">
                        <p class="eyebrow chunking-group-label">{group.label}</p>
                        <div class="chunking-card-grid">
                            {group.members.into_iter().map(|cc| view! {
                                <ChunkingCard cc=cc on_edit=on_edit on_delete=on_delete busy=busy />
                            }).collect_view()}
                        </div>
                    </section>
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

struct StrategyGroup {
    strategy: ChunkStrategy,
    label: &'static str,
    members: Vec<ChunkingConfigurationDto>,
}

impl StrategyGroup {
    fn empty(definition: &ChunkerDefinition) -> Self {
        Self {
            strategy: definition.strategy,
            label: definition.label,
            members: Vec::new(),
        }
    }
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
    let name_attr = name.clone();
    let is_default = cc.is_default;
    let card_class = if is_default {
        "chunking-card is-default"
    } else {
        "chunking-card"
    };
    let specs: Vec<(&'static str, u32)> = cc
        .config
        .strategy()
        .definition()
        .params
        .iter()
        .map(|param| (param.label, cc.config.param_value(param.key)))
        .collect();

    view! {
        <div class=card_class>
            <div class="chunking-card-head">
                <span class="chunking-card-name" title=name_attr>{name}</span>
                {is_default.then(|| view! {
                    <span class="eyebrow chunking-card-default">"default"</span>
                })}
            </div>
            <dl class="chunking-card-specs">
                {specs.into_iter().map(|(label, value)| view! {
                    <div class="chunking-spec">
                        <dt>{label}</dt>
                        <dd>{value}</dd>
                    </div>
                }).collect_view()}
            </dl>
            <div class="chunking-card-actions">
                <button
                    type="button"
                    class="btn btn-compact"
                    disabled=busy
                    on:click=move |_| on_edit.run(cc_edit.clone())
                >
                    "Edit"
                </button>
                <button
                    type="button"
                    class="btn btn-compact"
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
    set_status: WriteSignal<Option<InlineStatusMessage>>,
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
