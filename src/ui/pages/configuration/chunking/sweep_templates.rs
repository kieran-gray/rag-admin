use std::collections::HashMap;

use leptos::prelude::*;
use uuid::Uuid;

use crate::shared::contracts::{
    ChunkingConfigurationDto, CreateSweepTemplateDto, DeleteSweepTemplateDto,
    SweepTemplateCommandDto, SweepTemplateDto, UpdateSweepTemplateDto,
};
use crate::ui::components::primitives::{Dialog, EmptyState, InlineStatusMessage, Surface};
use crate::ui::pages::configuration::commands::run_sweep_template_command;

use super::widgets::LabelledInput;
use super::SweepFormMode;

#[component]
pub(super) fn SweepTemplateList(
    templates: Vec<SweepTemplateDto>,
    chunking_configs: Vec<ChunkingConfigurationDto>,
    on_edit: Callback<SweepTemplateDto>,
    on_delete: Callback<SweepTemplateDto>,
    on_set_default: Callback<SweepTemplateDto>,
    on_duplicate: Callback<SweepTemplateDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    if templates.is_empty() {
        return view! {
            <Surface>
                <EmptyState
                    title="No sweep templates yet"
                    body="A sweep template names a bundle of chunking configurations. Pick it when launching an evaluation to test those configurations side-by-side.".to_string()
                />
            </Surface>
        }
        .into_any();
    }

    let lookup: HashMap<Uuid, String> = chunking_configs
        .into_iter()
        .map(|cc| (cc.chunking_configuration_id, cc.name))
        .collect();
    let lookup = StoredValue::new(lookup);

    view! {
        <div class="space-y-3">
            {templates.into_iter().map(|st| view! {
                <SweepTemplateCard
                    template=st
                    lookup=lookup
                    on_edit=on_edit
                    on_delete=on_delete
                    on_set_default=on_set_default
                    on_duplicate=on_duplicate
                    busy=busy
                />
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn SweepTemplateCard(
    template: SweepTemplateDto,
    lookup: StoredValue<HashMap<Uuid, String>>,
    on_edit: Callback<SweepTemplateDto>,
    on_delete: Callback<SweepTemplateDto>,
    on_set_default: Callback<SweepTemplateDto>,
    on_duplicate: Callback<SweepTemplateDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let t_edit = template.clone();
    let t_delete = template.clone();
    let t_default = template.clone();
    let t_duplicate = template.clone();
    let name = template.name.clone();
    let is_default = template.is_default;
    let member_count = template.members.len();
    let member_names: Vec<String> = template
        .members
        .iter()
        .map(|id| {
            lookup
                .with_value(|m| m.get(id).cloned())
                .unwrap_or_else(|| format!("(missing {id})"))
        })
        .collect();

    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <div class="flex items-center gap-2">
                    <h3 class="section-title">{name}</h3>
                    {is_default.then(|| view! { <span class="pill pill-accent">"Default"</span> })}
                </div>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{format!("{member_count} configurations")}</span>
                    {member_names.into_iter().map(|n| view! {
                        <span class="pill pill-neutral">{n}</span>
                    }).collect_view()}
                </div>
            </div>
            <div class="flex gap-2 shrink-0 flex-wrap">
                <button
                    type="button"
                    class="btn"
                    disabled=Signal::derive(move || busy.get() || is_default)
                    on:click=move |_| on_set_default.run(t_default.clone())
                >
                    "Set default"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_duplicate.run(t_duplicate.clone())
                >
                    "Duplicate"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_edit.run(t_edit.clone())
                >
                    "Edit"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_delete.run(t_delete.clone())
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}

#[component]
pub(super) fn SweepTemplateFormDialog(
    form_mode: ReadSignal<Option<SweepFormMode>>,
    set_form_mode: WriteSignal<Option<SweepFormMode>>,
    chunking_configs_resource: Resource<Result<Vec<ChunkingConfigurationDto>, String>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (selected, set_selected) = signal::<Vec<Uuid>>(Vec::new());
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form_mode.get() {
            None => {}
            Some(SweepFormMode::Add) => {
                set_name.set(String::new());
                set_selected.set(Vec::new());
            }
            Some(SweepFormMode::Edit(t)) => {
                set_name.set(t.name);
                set_selected.set(t.members);
            }
        }
    });

    let close = Callback::new(move |_| {
        set_form_mode.set(None);
        set_dialog_error.set(None);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name_val = name.get().trim().to_string();
        if name_val.is_empty() {
            set_dialog_error.set(Some("Name is required.".into()));
            return;
        }
        let members = selected.get();
        if members.is_empty() {
            set_dialog_error.set(Some("Pick at least one chunking configuration.".into()));
            return;
        }
        let command = match form_mode.get() {
            Some(SweepFormMode::Add) => {
                SweepTemplateCommandDto::CreateSweepTemplate(CreateSweepTemplateDto {
                    name: name_val,
                    members,
                })
            }
            Some(SweepFormMode::Edit(t)) => {
                SweepTemplateCommandDto::UpdateSweepTemplate(UpdateSweepTemplateDto {
                    sweep_template_id: t.sweep_template_id,
                    name: name_val,
                    members,
                })
            }
            None => return,
        };
        run_sweep_template_command(
            command,
            "Sweep template saved",
            set_busy,
            set_status,
            Some(set_dialog_error),
            set_refresh,
            move || set_form_mode.set(None),
        );
    };

    view! {
        <Dialog
            open=Signal::derive(move || form_mode.get().is_some())
            title=Signal::derive(move || match form_mode.get() {
                Some(SweepFormMode::Edit(_)) => "Edit sweep template".to_string(),
                _ => "New sweep template".to_string(),
            })
            subtitle="Group chunking configurations under a name and use them as the variants axis on the evaluation launcher.".to_string()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|msg| view! {
                    <div class="log-line-error text-sm">{msg}</div>
                })}

                <LabelledInput
                    label="Name".to_string()
                    hint="e.g. bert-only-sweep, section-tight".to_string()
                    value=name
                    set_value=set_name
                />

                <div class="space-y-1.5">
                    <span class="eyebrow">"Chunking configurations"</span>
                    <SweepMemberPicker
                        chunking_configs_resource=chunking_configs_resource
                        selected=selected
                        set_selected=set_selected
                    />
                </div>

                <div class="flex justify-end gap-2 pt-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="submit" class="btn btn-primary" disabled=busy>
                        {move || if busy.get() { "Saving…" } else { "Save" }}
                    </button>
                </div>
            </form>
        </Dialog>
    }
}

#[component]
fn SweepMemberPicker(
    chunking_configs_resource: Resource<Result<Vec<ChunkingConfigurationDto>, String>>,
    selected: ReadSignal<Vec<Uuid>>,
    set_selected: WriteSignal<Vec<Uuid>>,
) -> impl IntoView {
    view! {
        <Transition fallback=|| view! { <p class="muted text-sm">"Loading chunking configurations…"</p> }>
            {move || match chunking_configs_resource.get() {
                Some(Ok(list)) if list.is_empty() => view! {
                    <p class="text-sm muted">"No chunking configurations available. Create some above first."</p>
                }.into_any(),
                Some(Ok(list)) => view! {
                    <div class="flex flex-col gap-1.5 max-h-64 overflow-y-auto">
                        {list.into_iter().map(|cc| {
                            let id = cc.chunking_configuration_id;
                            let label = cc.name.clone();
                            let descriptor = cc.config.describe();
                            let is_checked = move || selected.get().contains(&id);
                            view! {
                                <label class="flex items-center gap-2 px-2 py-1 rounded hover:bg-[var(--color-surface-2)] cursor-pointer">
                                    <input
                                        type="checkbox"
                                        prop:checked=is_checked
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            set_selected.update(|list| {
                                                if checked {
                                                    if !list.contains(&id) {
                                                        list.push(id);
                                                    }
                                                } else {
                                                    list.retain(|m| *m != id);
                                                }
                                            });
                                        }
                                    />
                                    <span class="text-sm text-text">{label}</span>
                                    <span class="text-xs faint">{descriptor}</span>
                                </label>
                            }
                        }).collect_view()}
                    </div>
                }.into_any(),
                Some(Err(e)) => view! {
                    <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                }.into_any(),
                None => ().into_any(),
            }}
        </Transition>
    }
}

#[component]
pub(super) fn SweepTemplateDeleteDialog(
    target: ReadSignal<Option<SweepTemplateDto>>,
    set_target: WriteSignal<Option<SweepTemplateDto>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(t) = target.get_untracked() else {
            return;
        };
        run_sweep_template_command(
            SweepTemplateCommandDto::DeleteSweepTemplate(DeleteSweepTemplateDto {
                sweep_template_id: t.sweep_template_id,
            }),
            "Sweep template deleted",
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
            title="Delete sweep template".to_string()
            subtitle="The underlying chunking configurations stay in place; only this named bundle is removed.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised p-3 rounded">
                    <span class="muted text-sm">"Template"</span>
                    <div class="text-text">{move || target.get().map(|t| t.name).unwrap_or_default()}</div>
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
