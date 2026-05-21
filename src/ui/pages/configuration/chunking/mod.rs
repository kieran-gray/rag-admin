mod form_dialog;
mod list;
mod sweep_templates;
mod widgets;

use leptos::prelude::*;

use crate::server_functions::configuration::{
    get_chunking_configurations, get_configuration, get_sweep_templates,
};
use crate::shared::contracts::{
    aggregate_type, ChunkingConfigurationDto, CreateSweepTemplateDto, SetDefaultSweepTemplateDto,
    SweepTemplateCommandDto, SweepTemplateDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{InlineStatusMessage, PageHeader, Surface};
use crate::ui::pages::configuration::commands::run_sweep_template_command;

use self::form_dialog::ChunkingFormDialog;
use self::list::{ChunkingList, DeleteConfirmDialog};
use self::sweep_templates::{
    SweepTemplateDeleteDialog, SweepTemplateFormDialog, SweepTemplateList,
};
use self::widgets::StatusBanner;

#[derive(Clone)]
pub(super) enum FormMode {
    Add,
    Edit(ChunkingConfigurationDto),
}

#[derive(Clone)]
pub(super) enum SweepFormMode {
    Add,
    Edit(SweepTemplateDto),
}

#[component]
pub fn ChunkingPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::GENERATION_MODEL_CATALOG,
            aggregate_type::SWEEP_TEMPLATE,
        ])
    });
    let (refresh, set_refresh) = signal(0u32);

    let configurations = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move {
            get_chunking_configurations()
                .await
                .map_err(|e| e.to_string())
        },
    );
    let configuration = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_configuration().await.map_err(|e| e.to_string()) },
    );
    let sweep_templates = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_sweep_templates().await.map_err(|e| e.to_string()) },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);
    let (form_mode, set_form_mode) = signal::<Option<FormMode>>(None);
    let (delete_target, set_delete_target) = signal::<Option<ChunkingConfigurationDto>>(None);
    let (sweep_form_mode, set_sweep_form_mode) = signal::<Option<SweepFormMode>>(None);
    let (sweep_delete_target, set_sweep_delete_target) = signal::<Option<SweepTemplateDto>>(None);

    view! {
        <div>
            <PageHeader
                title="Chunking"
                subtitle="Named chunking configurations. Pick a strategy, tune the parameters, and reuse them across ingestions and evaluations.".to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=move |_| set_form_mode.set(Some(FormMode::Add))
                    >
                        "+ New chunking config"
                    </button>
                }.into_any())
            />

            <StatusBanner status=status />

            <Transition fallback=|| view! { <p class="muted">"Loading chunking configurations…"</p> }>
                {move || {
                    let list = match configurations.get() {
                        Some(Ok(l)) => l,
                        Some(Err(e)) => {
                            return view! {
                                <Surface>
                                    <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                                </Surface>
                            }.into_any();
                        }
                        None => return ().into_any(),
                    };

                    view! {
                        <ChunkingList
                            configurations=list
                            on_edit=Callback::new(move |cc: ChunkingConfigurationDto| set_form_mode.set(Some(FormMode::Edit(cc))))
                            on_delete=Callback::new(move |cc: ChunkingConfigurationDto| set_delete_target.set(Some(cc)))
                            busy=busy
                        />
                    }.into_any()
                }}
            </Transition>

            <Transition fallback=|| ().into_any()>
                {move || configuration.get().map(|res| match res {
                    Ok(cfg) => view! {
                        <ChunkingFormDialog
                            config=cfg
                            form_mode=form_mode
                            set_form_mode=set_form_mode
                            busy=busy
                            set_busy=set_busy
                            set_status=set_status
                            set_refresh=set_refresh
                        />
                    }.into_any(),
                    Err(_) => ().into_any(),
                })}
            </Transition>

            <DeleteConfirmDialog
                target=delete_target
                set_target=set_delete_target
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />

            <div class="mt-8 mb-3 flex items-end justify-between gap-3">
                <div>
                    <h2 class="section-title">"Sweep templates"</h2>
                    <p class="text-sm muted">
                        "Named bundles of chunking configurations. Pick one as the variants axis when launching an evaluation."
                    </p>
                </div>
                <button
                    type="button"
                    class="btn btn-primary"
                    on:click=move |_| set_sweep_form_mode.set(Some(SweepFormMode::Add))
                >
                    "+ New sweep template"
                </button>
            </div>

            <Transition fallback=|| view! { <p class="muted">"Loading sweep templates…"</p> }>
                {move || {
                    let templates = match sweep_templates.get() {
                        Some(Ok(t)) => t,
                        Some(Err(e)) => {
                            return view! {
                                <Surface>
                                    <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                                </Surface>
                            }.into_any();
                        }
                        None => return ().into_any(),
                    };
                    let chunking_configs = configurations.get().and_then(Result::ok).unwrap_or_default();

                    view! {
                        <SweepTemplateList
                            templates=templates
                            chunking_configs=chunking_configs
                            on_edit=Callback::new(move |st: SweepTemplateDto| set_sweep_form_mode.set(Some(SweepFormMode::Edit(st))))
                            on_delete=Callback::new(move |st: SweepTemplateDto| set_sweep_delete_target.set(Some(st)))
                            on_set_default=Callback::new(move |st: SweepTemplateDto| {
                                run_sweep_template_command(
                                    SweepTemplateCommandDto::SetDefaultSweepTemplate(SetDefaultSweepTemplateDto {
                                        sweep_template_id: st.sweep_template_id,
                                    }),
                                    "Default sweep template set",
                                    set_busy,
                                    set_status,
                                    None,
                                    set_refresh,
                                    || {},
                                );
                            })
                            on_duplicate=Callback::new(move |st: SweepTemplateDto| {
                                run_sweep_template_command(
                                    SweepTemplateCommandDto::CreateSweepTemplate(CreateSweepTemplateDto {
                                        name: format!("{}-copy", st.name),
                                        members: st.members,
                                    }),
                                    "Sweep template duplicated",
                                    set_busy,
                                    set_status,
                                    None,
                                    set_refresh,
                                    || {},
                                );
                            })
                            busy=busy
                        />
                    }.into_any()
                }}
            </Transition>

            <SweepTemplateFormDialog
                form_mode=sweep_form_mode
                set_form_mode=set_sweep_form_mode
                chunking_configs_resource=configurations
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />

            <SweepTemplateDeleteDialog
                target=sweep_delete_target
                set_target=set_sweep_delete_target
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />
        </div>
    }
}
