mod form_dialog;

use leptos::prelude::*;

use crate::server_functions::connector::list_connectors;
use crate::shared::contracts::{
    aggregate_type, ConnectorCommandDto, ConnectorConfigDto, ConnectorDto, UnregisterConnectorDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{
    Dialog, EmptyState, InlineStatus, InlineStatusMessage, PageHeader, Surface,
};

use self::form_dialog::{ConnectorForm, ConnectorFormDialog};

#[component]
pub fn ConnectorsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::CONNECTOR]));
    let (refresh, set_refresh) = signal(0u32);

    let connectors = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { list_connectors().await.map_err(|e| e.to_string()) },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);
    let (form, set_form) = signal::<Option<ConnectorForm>>(None);
    let (delete_target, set_delete_target) = signal::<Option<ConnectorDto>>(None);

    let open_new = move |_| set_form.set(Some(ConnectorForm::Add));
    let open_edit =
        Callback::new(move |c: ConnectorDto| set_form.set(Some(ConnectorForm::Edit(c))));
    let open_delete = Callback::new(move |c: ConnectorDto| set_delete_target.set(Some(c)));

    view! {
        <div>
            <PageHeader
                title="Connectors"
                subtitle="Configured integrations that discover and ingest documents from upstream sources.".to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=open_new
                    >
                        "+ Add connector"
                    </button>
                }.into_any())
            />

            <InlineStatus status=status />

            <Suspense fallback=|| view! {
                <Surface><div class="p-6 muted text-sm">"Loading connectors…"</div></Surface>
            }>
                {move || connectors.get().map(|res| match res {
                    Err(e) => view! {
                        <Surface>
                            <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                        </Surface>
                    }.into_any(),
                    Ok(list) if list.is_empty() => view! {
                        <Surface>
                            <EmptyState
                                title="No connectors configured"
                                body="A connector defines an upstream source (like a sitemap or RSS feed) you can pull documents from. Add one to enable discovery in the import dialog.".to_string()
                            />
                        </Surface>
                    }.into_any(),
                    Ok(list) => view! {
                        <div class="space-y-3">
                            {list.into_iter().map(|c| view! {
                                <ConnectorCard
                                    connector=c
                                    on_edit=open_edit
                                    on_delete=open_delete
                                    busy=busy
                                />
                            }).collect_view()}
                        </div>
                    }.into_any(),
                })}
            </Suspense>

            <ConnectorFormDialog
                form=form
                set_form=set_form
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />

            <ConnectorDeleteDialog
                target=delete_target
                set_target=set_delete_target
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />
        </div>
    }
}

#[component]
fn ConnectorCard(
    connector: ConnectorDto,
    on_edit: Callback<ConnectorDto>,
    on_delete: Callback<ConnectorDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let edit_clone = connector.clone();
    let delete_clone = connector.clone();
    let name = connector.name.clone();
    let kind_label = connector.config.kind().display_label();
    let descriptor = match &connector.config {
        ConnectorConfigDto::Sitemap(c) => c.url.clone(),
    };

    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <h3 class="section-title flex items-center gap-2">{name}</h3>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{kind_label}</span>
                    <span class="pill pill-neutral truncate max-w-md">{descriptor}</span>
                </div>
            </div>
            <div class="flex gap-2 shrink-0">
                <a
                    class="btn"
                    href=format!("/documents?tab=connector:{}", connector.connector_id)
                >
                    "Browse"
                </a>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_edit.run(edit_clone.clone())
                >
                    "Edit"
                </button>
                <button
                    type="button"
                    class="btn"
                    disabled=busy
                    on:click=move |_| on_delete.run(delete_clone.clone())
                >
                    "Delete"
                </button>
            </div>
        </div>
    }
}

#[component]
fn ConnectorDeleteDialog(
    target: ReadSignal<Option<ConnectorDto>>,
    set_target: WriteSignal<Option<ConnectorDto>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(c) = target.get_untracked() else {
            return;
        };
        run_connector_command(
            ConnectorCommandDto::UnregisterConnector(UnregisterConnectorDto {
                connector_id: c.connector_id,
            }),
            "Connector deleted",
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
            title="Delete connector"
            subtitle="This connector and its discovery configuration will be removed. Imported documents are not affected.".to_string()
            on_close=close
        >
            {move || target.get().map(|c| view! {
                <div class="space-y-3">
                    <p class="text-sm">"Delete "<strong>{c.name.clone()}</strong>"?"</p>
                    <div class="flex justify-end gap-2">
                        <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                            "Cancel"
                        </button>
                        <button type="button" class="btn btn-primary" disabled=busy on:click=confirm>
                            {move || if busy.get() { "Deleting…" } else { "Delete" }}
                        </button>
                    </div>
                </div>
            })}
        </Dialog>
    }
}

pub(crate) fn run_connector_command<F>(
    command: ConnectorCommandDto,
    success_message: &'static str,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    dialog_status: Option<WriteSignal<Option<String>>>,
    set_refresh: WriteSignal<u32>,
    on_success: F,
) where
    F: FnOnce() + 'static,
{
    use crate::server_functions::connector::apply_connector_command;
    use leptos::task::spawn_local;

    set_busy.set(true);
    set_status.set(None);
    if let Some(ds) = dialog_status {
        ds.set(None);
    }
    spawn_local(async move {
        match apply_connector_command(command).await {
            Ok(_) => {
                if let Some(ds) = dialog_status {
                    ds.set(None);
                }
                on_success();
                set_status.set(Some(InlineStatusMessage::ok(success_message)));
                set_refresh.update(|v| *v += 1);
            }
            Err(e) => {
                let message = format!("COMMAND_FAULT: {e}");
                if let Some(ds) = dialog_status {
                    ds.set(Some(message));
                } else {
                    set_status.set(Some(InlineStatusMessage::err(message)));
                }
            }
        }
        set_busy.set(false);
    });
}
