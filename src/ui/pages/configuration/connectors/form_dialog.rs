use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::server_functions::configuration::{
    get_chunking_configurations, get_index_profiles, get_retrieval_profiles,
};
use crate::server_functions::connector::apply_connector_command;
use crate::shared::contracts::{
    ChunkingConfigurationDto, ConnectorCommandDto, ConnectorConfigDto, ConnectorDto,
    ConnectorKindDto, IndexProfileDto, RegisterConnectorDto, RetrievalProfileDto,
    SetConnectorDefaultsDto, SitemapConfigDto, UpdateConnectorConfigDto,
};
use crate::ui::components::primitives::{Dialog, InlineStatusMessage};

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
    set_status: WriteSignal<Option<InlineStatusMessage>>,
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
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
    on_close: Callback<()>,
) -> impl IntoView {
    let (
        name_initial,
        sitemap_initial,
        edit_id,
        index_profile_initial,
        chunking_initial,
        retrieval_profile_initial,
    ) = match &form {
        ConnectorForm::Add => (
            String::new(),
            SitemapConfigDto {
                url: String::new(),
                include_patterns: vec![],
                exclude_patterns: vec![],
            },
            None,
            None,
            None,
            None,
        ),
        ConnectorForm::Edit(c) => match &c.config {
            ConnectorConfigDto::Sitemap(s) => (
                c.name.clone(),
                s.clone(),
                Some(c.connector_id),
                c.default_index_profile_id,
                c.default_chunking_configuration_id,
                c.default_retrieval_profile_id,
            ),
        },
    };

    let (name, set_name) = signal(name_initial);
    let (url, set_url) = signal(sitemap_initial.url);
    let (include, set_include) = signal(sitemap_initial.include_patterns.join("\n"));
    let (exclude, set_exclude) = signal(sitemap_initial.exclude_patterns.join("\n"));
    let (index_profile_default, set_index_profile_default) = signal(index_profile_initial);
    let (chunking_default, set_chunking_default) = signal(chunking_initial);
    let (retrieval_profile_default, set_retrieval_profile_default) =
        signal(retrieval_profile_initial);
    let (dialog_status, set_dialog_status) = signal::<Option<String>>(None);
    let is_edit = edit_id.is_some();

    let index_profiles = Resource::new(
        || (),
        |_| async move { get_index_profiles().await.map_err(|e| e.to_string()) },
    );
    let retrieval_profiles = Resource::new(
        || (),
        |_| async move { get_retrieval_profiles().await.map_err(|e| e.to_string()) },
    );
    let chunkings = Resource::new(
        || (),
        |_| async move {
            get_chunking_configurations()
                .await
                .map_err(|e| e.to_string())
        },
    );

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

        let primary = match edit_id {
            None => ConnectorCommandDto::RegisterConnector(RegisterConnectorDto {
                name: name_value,
                config,
            }),
            Some(id) => ConnectorCommandDto::UpdateConnectorConfig(UpdateConnectorConfigDto {
                connector_id: id,
                config,
            }),
        };

        let index_profile_default_value = index_profile_default.get_untracked();
        let chunking_default_value = chunking_default.get_untracked();
        let retrieval_profile_default_value = retrieval_profile_default.get_untracked();
        let success_message = if is_edit {
            "Connector updated"
        } else {
            "Connector created"
        };

        set_busy.set(true);
        set_status.set(None);
        set_dialog_status.set(None);

        spawn_local(async move {
            let primary_result = apply_connector_command(primary).await;
            let resolved_id = match primary_result {
                Ok(dto) => dto.as_ref().map(|d| d.connector_id).or(edit_id),
                Err(e) => {
                    set_dialog_status.set(Some(format!("COMMAND_FAULT: {e}")));
                    set_busy.set(false);
                    return;
                }
            };

            if let Some(connector_id) = resolved_id {
                let defaults_cmd =
                    ConnectorCommandDto::SetConnectorDefaults(SetConnectorDefaultsDto {
                        connector_id,
                        index_profile_id: index_profile_default_value,
                        chunking_configuration_id: chunking_default_value,
                        retrieval_profile_id: retrieval_profile_default_value,
                    });
                if let Err(e) = apply_connector_command(defaults_cmd).await {
                    set_dialog_status.set(Some(format!("COMMAND_FAULT (defaults): {e}")));
                    set_busy.set(false);
                    return;
                }
            }

            set_status.set(Some(InlineStatusMessage::ok(success_message)));
            set_refresh.update(|v| *v += 1);
            on_close.run(());
            set_busy.set(false);
        });
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

            <DefaultsSection
                index_profiles=index_profiles
                retrieval_profiles=retrieval_profiles
                chunkings=chunkings
                index_profile_default=index_profile_default
                set_index_profile_default=set_index_profile_default
                retrieval_profile_default=retrieval_profile_default
                set_retrieval_profile_default=set_retrieval_profile_default
                chunking_default=chunking_default
                set_chunking_default=set_chunking_default
            />

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

#[component]
#[allow(clippy::too_many_arguments)]
fn DefaultsSection(
    index_profiles: Resource<Result<Vec<IndexProfileDto>, String>>,
    retrieval_profiles: Resource<Result<Vec<RetrievalProfileDto>, String>>,
    chunkings: Resource<Result<Vec<ChunkingConfigurationDto>, String>>,
    index_profile_default: ReadSignal<Option<Uuid>>,
    set_index_profile_default: WriteSignal<Option<Uuid>>,
    retrieval_profile_default: ReadSignal<Option<Uuid>>,
    set_retrieval_profile_default: WriteSignal<Option<Uuid>>,
    chunking_default: ReadSignal<Option<Uuid>>,
    set_chunking_default: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    view! {
        <fieldset class="flex flex-col gap-3 border border-[var(--color-border)] rounded p-3">
            <legend class="text-xs uppercase tracking-wide muted px-1">"Connector defaults"</legend>
            <p class="text-xs muted">
                "Documents imported via this connector use these defaults. Leave blank to use the application defaults."
            </p>

            <label class="flex flex-col gap-1 text-sm">
                <span class="muted">"Default index profile"</span>
                <Suspense fallback=|| view! { <span class="faint text-xs">"Loading index profiles…"</span> }>
                    {move || index_profiles.get().map(|res| match res {
                        Err(e) => view! { <div class="log-line-error text-xs">{e}</div> }.into_any(),
                        Ok(list) => view! {
                            <select
                                class="input"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_index_profile_default.set(Uuid::parse_str(&value).ok());
                                }
                            >
                                <option value="" selected=move || index_profile_default.get().is_none()>
                                    "— Use application default —"
                                </option>
                                {list.into_iter().map(|p| {
                                    let id = p.index_profile_id;
                                    let is_selected = index_profile_default.get_untracked() == Some(id);
                                    let label = if p.is_default {
                                        format!("{} (app default)", p.name)
                                    } else {
                                        p.name
                                    };
                                    view! {
                                        <option value=id.to_string() selected=is_selected>
                                            {label}
                                        </option>
                                    }
                                }).collect_view()}
                            </select>
                        }.into_any(),
                    })}
                </Suspense>
            </label>

            <label class="flex flex-col gap-1 text-sm">
                <span class="muted">"Default retrieval profile"</span>
                <Suspense fallback=|| view! { <span class="faint text-xs">"Loading retrieval profiles…"</span> }>
                    {move || retrieval_profiles.get().map(|res| match res {
                        Err(e) => view! { <div class="log-line-error text-xs">{e}</div> }.into_any(),
                        Ok(list) => view! {
                            <select
                                class="input"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_retrieval_profile_default.set(Uuid::parse_str(&value).ok());
                                }
                            >
                                <option value="" selected=move || retrieval_profile_default.get().is_none()>
                                    "— Use application default —"
                                </option>
                                {list.into_iter().map(|p| {
                                    let id = p.retrieval_profile_id;
                                    let is_selected = retrieval_profile_default.get_untracked() == Some(id);
                                    let label = if p.is_default {
                                        format!("{} (app default)", p.name)
                                    } else {
                                        p.name
                                    };
                                    view! {
                                        <option value=id.to_string() selected=is_selected>
                                            {label}
                                        </option>
                                    }
                                }).collect_view()}
                            </select>
                        }.into_any(),
                    })}
                </Suspense>
            </label>

            <label class="flex flex-col gap-1 text-sm">
                <span class="muted">"Default chunking"</span>
                <Suspense fallback=|| view! { <span class="faint text-xs">"Loading chunking configurations…"</span> }>
                    {move || chunkings.get().map(|res| match res {
                        Err(e) => view! { <div class="log-line-error text-xs">{e}</div> }.into_any(),
                        Ok(list) => view! {
                            <select
                                class="input"
                                on:change=move |ev| {
                                    let value = event_target_value(&ev);
                                    set_chunking_default.set(Uuid::parse_str(&value).ok());
                                }
                            >
                                <option value="" selected=move || chunking_default.get().is_none()>
                                    "— Use application default —"
                                </option>
                                {list.into_iter().map(|c| {
                                    let id = c.chunking_configuration_id;
                                    let is_selected = chunking_default.get_untracked() == Some(id);
                                    let label = if c.is_default {
                                        format!("{} (app default)", c.name)
                                    } else {
                                        c.name
                                    };
                                    view! {
                                        <option value=id.to_string() selected=is_selected>
                                            {label}
                                        </option>
                                    }
                                }).collect_view()}
                            </select>
                        }.into_any(),
                    })}
                </Suspense>
            </label>
        </fieldset>
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
