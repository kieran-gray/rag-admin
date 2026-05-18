use leptos::ev::SubmitEvent;
use leptos::prelude::*;

use crate::shared::contracts::{
    ConnectorCommandDto, ConnectorConfigDto, ConnectorDto, ConnectorKindDto, RegisterConnectorDto,
    SitemapConfigDto, UpdateConnectorConfigDto,
};
use crate::ui::components::primitives::Dialog;

use super::run_connector_command;

#[derive(Clone)]
pub(super) enum ConnectorForm {
    Add,
    Edit(ConnectorDto),
}

#[component]
pub(super) fn ConnectorFormDialog(
    form: ReadSignal<Option<ConnectorForm>>,
    set_form: WriteSignal<Option<ConnectorForm>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_form.set(None));
    let open = Signal::derive(move || form.get().is_some());

    view! {
        <Dialog
            open=open
            title="Connector"
            subtitle="A connector points at an upstream source that exposes a discoverable list of documents.".to_string()
            on_close=close
        >
            {move || form.get().map(|f| view! {
                <ConnectorFormBody
                    form=f
                    busy=busy
                    set_busy=set_busy
                    set_status=set_status
                    set_refresh=set_refresh
                    on_close=close
                />
            })}
        </Dialog>
    }
}

#[component]
fn ConnectorFormBody(
    form: ConnectorForm,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<(bool, String)>>,
    set_refresh: WriteSignal<u32>,
    on_close: Callback<()>,
) -> impl IntoView {
    let (name_initial, sitemap_initial, edit_id) = match &form {
        ConnectorForm::Add => (
            String::new(),
            SitemapConfigDto {
                url: String::new(),
                include_patterns: vec![],
                exclude_patterns: vec![],
            },
            None,
        ),
        ConnectorForm::Edit(c) => match &c.config {
            ConnectorConfigDto::Sitemap(s) => (c.name.clone(), s.clone(), Some(c.connector_id)),
        },
    };

    let (name, set_name) = signal(name_initial);
    let (url, set_url) = signal(sitemap_initial.url);
    let (include, set_include) = signal(sitemap_initial.include_patterns.join("\n"));
    let (exclude, set_exclude) = signal(sitemap_initial.exclude_patterns.join("\n"));
    let (dialog_status, set_dialog_status) = signal::<Option<String>>(None);
    let is_edit = edit_id.is_some();

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let name_value = name.get_untracked().trim().to_string();
        let url_value = url.get_untracked().trim().to_string();
        if name_value.is_empty() {
            set_dialog_status.set(Some("Name is required".into()));
            return;
        }
        if url_value.is_empty() {
            set_dialog_status.set(Some("Sitemap URL is required".into()));
            return;
        }
        let config = ConnectorConfigDto::Sitemap(SitemapConfigDto {
            url: url_value,
            include_patterns: split_lines(&include.get_untracked()),
            exclude_patterns: split_lines(&exclude.get_untracked()),
        });

        let command = match edit_id {
            None => ConnectorCommandDto::RegisterConnector(RegisterConnectorDto {
                name: name_value,
                config,
            }),
            Some(id) => ConnectorCommandDto::UpdateConnectorConfig(UpdateConnectorConfigDto {
                connector_id: id,
                config,
            }),
        };

        run_connector_command(
            command,
            if is_edit {
                "Connector updated"
            } else {
                "Connector created"
            },
            set_busy,
            set_status,
            Some(set_dialog_status),
            set_refresh,
            move || on_close.run(()),
        );
    };

    view! {
        <form on:submit=submit class="flex flex-col gap-3">
            <label class="flex flex-col gap-1 text-sm">
                <span class="muted">"Name"</span>
                <input
                    type="text"
                    class="input"
                    placeholder="My docs site"
                    prop:value=name
                    on:input=move |ev| set_name.set(event_target_value(&ev))
                />
            </label>

            <KindReadonly kind=ConnectorKindDto::Sitemap />

            <label class="flex flex-col gap-1 text-sm">
                <span class="muted">"Sitemap URL"</span>
                <input
                    type="url"
                    class="input"
                    placeholder="https://example.com/sitemap.xml"
                    prop:value=url
                    on:input=move |ev| set_url.set(event_target_value(&ev))
                />
            </label>

            <label class="flex flex-col gap-1 text-sm">
                <span class="muted">"Include patterns (optional, one per line)"</span>
                <textarea
                    class="input"
                    rows="3"
                    placeholder="/blog/&#10;/docs/"
                    prop:value=include
                    on:input=move |ev| set_include.set(event_target_value(&ev))
                />
            </label>

            <label class="flex flex-col gap-1 text-sm">
                <span class="muted">"Exclude patterns (optional, one per line)"</span>
                <textarea
                    class="input"
                    rows="3"
                    placeholder="/draft/&#10;/tag/"
                    prop:value=exclude
                    on:input=move |ev| set_exclude.set(event_target_value(&ev))
                />
            </label>

            {move || dialog_status.get().map(|s| view! {
                <div class="log-line-error text-sm">{s}</div>
            })}

            <div class="flex justify-end gap-2">
                <button type="button" class="btn" disabled=busy on:click=move |_| on_close.run(())>
                    "Cancel"
                </button>
                <button type="submit" class="btn btn-primary" disabled=busy>
                    {move || if busy.get() { "Saving…" } else if is_edit { "Save" } else { "Add" }}
                </button>
            </div>
        </form>
    }
}

#[component]
fn KindReadonly(kind: ConnectorKindDto) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-1 text-sm">
            <span class="muted">"Kind"</span>
            <span class="pill pill-neutral self-start">{kind.display_label()}</span>
        </div>
    }
}

fn split_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
