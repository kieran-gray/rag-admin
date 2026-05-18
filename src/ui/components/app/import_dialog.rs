use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use uuid::Uuid;
use wasm_bindgen::JsCast;

use crate::server_functions::connector::list_connectors;
use crate::server_functions::source_document::{
    import_from_connector, import_source_document_from_url, list_from_connector,
    start_indexing_with_defaults,
};
use crate::shared::contracts::{
    aggregate_type, ConnectorDiscoveredItemDto, ConnectorDto, SourceDocumentDto,
};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::primitives::{Dialog, Help, Status, StatusPill};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Upload,
    Url,
    Connector,
}

#[component]
pub fn ImportDialog(#[prop(into)] open: Signal<bool>, on_close: Callback<()>) -> impl IntoView {
    let (tab, set_tab) = signal(Tab::Upload);

    let on_imported_close = on_close;
    let handle_imported = Callback::new(move |dto: SourceDocumentDto| {
        let href = format!(
            "/documents/{}/{}",
            dto.document_type.to_lowercase(),
            urlencoding::encode(&dto.source_ref_key)
        );
        let navigate = use_navigate();
        on_imported_close.run(());
        navigate(&href, NavigateOptions::default());
    });

    view! {
        <Dialog
            open=open
            title="Import document"
            subtitle="Upload a file, paste a URL, or pick from a configured connector.".to_string()
            on_close=on_close
        >
            <nav class="border-b border-[var(--color-border)] mb-4 flex gap-1 -mt-1">
                <TabButton label="Upload"
                    active=move || tab.get() == Tab::Upload
                    on_click=Callback::new(move |_| set_tab.set(Tab::Upload)) />
                <TabButton label="From URL"
                    active=move || tab.get() == Tab::Url
                    on_click=Callback::new(move |_| set_tab.set(Tab::Url)) />
                <TabButton label="From connector"
                    active=move || tab.get() == Tab::Connector
                    on_click=Callback::new(move |_| set_tab.set(Tab::Connector)) />
            </nav>

            {move || match tab.get() {
                Tab::Upload => view! { <UploadPane on_imported=handle_imported /> }.into_any(),
                Tab::Url => view! { <UrlPane on_imported=handle_imported /> }.into_any(),
                Tab::Connector => view! { <ConnectorPane on_imported=handle_imported /> }.into_any(),
            }}
        </Dialog>
    }
}

#[component]
fn TabButton(
    label: &'static str,
    active: impl Fn() -> bool + Send + Sync + 'static,
    on_click: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || format!(
                "px-4 py-2 -mb-px border-b-2 text-sm font-medium transition-colors {}",
                if active() {
                    "border-[var(--color-accent)] text-text"
                } else {
                    "border-transparent muted hover:text-text"
                }
            )
            on:click=move |_| on_click.run(())
        >
            {label}
        </button>
    }
}

#[component]
fn IndexAfterImportCheckbox(
    index_after: ReadSignal<bool>,
    set_index_after: WriteSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-1">
            <label class="flex items-center gap-2 text-sm">
                <input
                    type="checkbox"
                    prop:checked=index_after
                    on:change=move |ev| set_index_after.set(event_target_checked(&ev))
                />
                <span>"Index immediately with default pipeline"</span>
            </label>
            <Help title="What does this do?".to_string()>
                <p>
                    "If enabled, the document is chunked, embedded, and indexed using the pipeline marked as default. You can change the default pipeline from the Configuration page."
                </p>
                <p class="mt-3">
                    "Leave unchecked to walk through chunking, embedding, and indexing step by step after import. Recommended on first use so you can preview chunks before vectorizing."
                </p>
            </Help>
        </div>
    }
}

#[component]
fn UploadPane(on_imported: Callback<SourceDocumentDto>) -> impl IntoView {
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (selected_name, set_selected_name) = signal::<Option<String>>(None);
    let (index_after, set_index_after) = signal(false);
    let (is_dragging, set_is_dragging) = signal(false);
    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();

    let upload_with_file = move |file: web_sys::File| {
        let auto_index = index_after.get_untracked();
        set_selected_name.set(Some(file.name()));
        set_busy.set(true);
        set_error.set(None);

        spawn_local(async move {
            match upload_file(file).await {
                Ok(dto) => {
                    if auto_index {
                        if let Err(e) =
                            start_indexing_with_defaults(dto.source_ref_key.clone()).await
                        {
                            set_error
                                .set(Some(format!("Imported, but indexing failed to start: {e}")));
                            set_busy.set(false);
                            return;
                        }
                    }
                    on_imported.run(dto);
                }
                Err(e) => {
                    set_error.set(Some(e));
                }
            }
            set_busy.set(false);
        });
    };

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let Some(input) = file_input.get_untracked() else {
            return;
        };
        let files = match input.files() {
            Some(f) if f.length() > 0 => f,
            _ => {
                set_error.set(Some("Choose a file first.".into()));
                return;
            }
        };
        let Some(file) = files.item(0) else {
            set_error.set(Some("Could not read selected file.".into()));
            return;
        };
        upload_with_file(file);
    };

    let on_dragover = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        if !is_dragging.get_untracked() {
            set_is_dragging.set(true);
        }
    };
    let on_dragleave = move |_ev: leptos::ev::DragEvent| {
        set_is_dragging.set(false);
    };
    let on_drop = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        set_is_dragging.set(false);
        if busy.get_untracked() {
            return;
        }
        let Some(dt) = ev.data_transfer() else { return };
        let Some(files) = dt.files() else { return };
        if files.length() == 0 {
            return;
        }
        let Some(file) = files.item(0) else { return };
        upload_with_file(file);
    };

    view! {
        <form on:submit=submit class="flex flex-col gap-3">
                <label
                    class="upload-dropzone"
                    class:is-dragging=is_dragging
                    on:dragover=on_dragover
                    on:dragleave=on_dragleave
                    on:drop=on_drop
                >
                    <div class="upload-dropzone-icon">"⤓"</div>
                    <div class="upload-dropzone-primary">
                        {move || if busy.get() {
                            "Uploading…".to_string()
                        } else {
                            "Drop a file here, or click to choose".to_string()
                        }}
                    </div>
                    <div class="upload-dropzone-secondary">
                        ".md, .markdown, .txt"
                    </div>
                    {move || selected_name.get().map(|n| view! {
                        <div class="upload-dropzone-selected">{format!("Selected: {n}")}</div>
                    })}
                    <input
                        type="file"
                        accept=".md,.markdown,.txt,text/markdown,text/plain"
                        class="hidden"
                        node_ref=file_input
                        on:change=move |ev| {
                            let target = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
                            let name = target
                                .and_then(|el| el.files())
                                .and_then(|f| f.item(0))
                                .map(|f| f.name());
                            set_selected_name.set(name);
                        }
                    />
                </label>

            <IndexAfterImportCheckbox index_after=index_after set_index_after=set_index_after />

            {move || error.get().map(|e| view! { <div class="log-line-error text-sm">{e}</div> })}

            <div class="flex justify-end">
                <button type="submit" class="btn btn-primary" disabled=busy>
                    {move || if busy.get() { "Uploading…" } else { "Upload" }}
                </button>
            </div>
        </form>
    }
}

#[component]
fn UrlPane(on_imported: Callback<SourceDocumentDto>) -> impl IntoView {
    let (url, set_url) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (index_after, set_index_after) = signal(false);

    let submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let value = url.get_untracked().trim().to_string();
        if value.is_empty() {
            set_error.set(Some("Enter a URL.".into()));
            return;
        }
        let auto_index = index_after.get_untracked();
        set_busy.set(true);
        set_error.set(None);

        spawn_local(async move {
            match import_source_document_from_url(value).await {
                Ok(dto) => {
                    if auto_index {
                        if let Err(e) =
                            start_indexing_with_defaults(dto.source_ref_key.clone()).await
                        {
                            set_error
                                .set(Some(format!("Imported, but indexing failed to start: {e}")));
                            set_busy.set(false);
                            return;
                        }
                    }
                    on_imported.run(dto);
                }
                Err(e) => set_error.set(Some(format!("{e}"))),
            }
            set_busy.set(false);
        });
    };

    view! {
        <form on:submit=submit class="flex flex-col gap-3">
            <label class="flex flex-col gap-1 text-sm">
                <span class="flex items-center muted">
                    "URL"
                    <Help title="How URL import works".to_string()>
                        <p>
                            "The server fetches the page and converts the rendered HTML to Markdown using readability heuristics. Headings, paragraphs, lists, and code blocks are preserved. Boilerplate (navigation, footers, ads) is stripped."
                        </p>
                        <p class="mt-3">
                            "If the page requires authentication or runs heavy JavaScript, conversion may be incomplete. Download the file and use the Upload tab instead."
                        </p>
                    </Help>
                </span>
                <input
                    type="url"
                    class="input"
                    placeholder="https://example.com/article"
                    prop:value=url
                    on:input=move |ev| set_url.set(event_target_value(&ev))
                />
            </label>

            <IndexAfterImportCheckbox index_after=index_after set_index_after=set_index_after />

            {move || error.get().map(|e| view! { <div class="log-line-error text-sm">{e}</div> })}

            <div class="flex justify-end">
                <button type="submit" class="btn btn-primary" disabled=busy>
                    {move || if busy.get() { "Fetching…" } else { "Import" }}
                </button>
            </div>
        </form>
    }
}

#[component]
fn ConnectorPane(on_imported: Callback<SourceDocumentDto>) -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::SOURCE_DOCUMENT,
            aggregate_type::INDEXING,
            aggregate_type::CONNECTOR,
        ])
    });
    let connectors = Resource::new(
        move || invalidator.get(),
        |_| async move { list_connectors().await },
    );

    let (selected, set_selected) = signal::<Option<Uuid>>(None);

    view! {
        <Suspense fallback=|| view! { <p class="muted text-sm">"Loading connectors…"</p> }>
            {move || connectors.get().map(|res| match res {
                Err(e) => view! {
                    <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                }.into_any(),
                Ok(list) if list.is_empty() => view! {
                    <p class="muted text-sm">
                        "No connectors are configured. Add one from "
                        <a href="/configuration/connectors" class="link">"Configuration → Connectors"</a>
                        "."
                    </p>
                }.into_any(),
                Ok(list) => {
                    if selected.get().is_none() {
                        if let Some(first) = list.first() {
                            set_selected.set(Some(first.connector_id));
                        }
                    }
                    view! {
                        <div class="flex flex-col gap-3">
                            <ConnectorSelect connectors=list.clone() selected=selected set_selected=set_selected />
                            <ConnectorItems
                                connectors=list
                                selected=selected
                                on_imported=on_imported
                            />
                        </div>
                    }.into_any()
                }
            })}
        </Suspense>
    }
}

#[component]
fn ConnectorSelect(
    connectors: Vec<ConnectorDto>,
    selected: ReadSignal<Option<Uuid>>,
    set_selected: WriteSignal<Option<Uuid>>,
) -> impl IntoView {
    let options = connectors.clone();
    view! {
        <label class="flex flex-col gap-1 text-sm">
            <span class="muted">"Connector"</span>
            <select
                class="input"
                on:change=move |ev| {
                    let value = event_target_value(&ev);
                    set_selected.set(Uuid::parse_str(&value).ok());
                }
            >
                {options.into_iter().map(|c| {
                    let id = c.connector_id.to_string();
                    let is_selected = selected.get_untracked() == Some(c.connector_id);
                    view! {
                        <option value=id selected=is_selected>
                            {c.name}
                        </option>
                    }
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn ConnectorItems(
    connectors: Vec<ConnectorDto>,
    selected: ReadSignal<Option<Uuid>>,
    on_imported: Callback<SourceDocumentDto>,
) -> impl IntoView {
    let connectors = StoredValue::new(connectors);
    let imports_invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::SOURCE_DOCUMENT]));
    let items = Resource::new(
        move || (selected.get(), imports_invalidator.get()),
        move |(id, _)| async move {
            match id {
                Some(id) => list_from_connector(id).await.map_err(|e| e.to_string()),
                None => Ok(Vec::new()),
            }
        },
    );

    view! {
        <Suspense fallback=|| view! { <p class="muted text-sm">"Discovering documents…"</p> }>
            {move || items.get().map(|res| match res {
                Err(e) => view! {
                    <div class="log-line-error text-sm">{format!("Failed: {e}")}</div>
                }.into_any(),
                Ok(list) if list.is_empty() => view! {
                    <p class="muted text-sm">"No items discovered from this connector."</p>
                }.into_any(),
                Ok(list) => {
                    let connector_id = selected.get_untracked();
                    let _ = connectors.get_value();
                    view! {
                        <ul class="flex flex-col gap-1 max-h-96 overflow-y-auto">
                            {list.into_iter().map(|item| view! {
                                <ConnectorItemRow
                                    connector_id=connector_id
                                    item=item
                                    on_imported=on_imported
                                />
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }
            })}
        </Suspense>
    }
}

#[component]
fn ConnectorItemRow(
    connector_id: Option<Uuid>,
    item: ConnectorDiscoveredItemDto,
    on_imported: Callback<SourceDocumentDto>,
) -> impl IntoView {
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let already_imported = item.already_imported;
    let source_ref_key = StoredValue::new(item.source_ref_key.clone());
    let title = item.title.clone();
    let key = item.source_ref_key.clone();

    let do_import = move |_| {
        let Some(cid) = connector_id else {
            return;
        };
        if busy.get_untracked() {
            return;
        }
        let key = source_ref_key.get_value();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match import_from_connector(cid, key).await {
                Ok(dto) => on_imported.run(dto),
                Err(e) => set_error.set(Some(format!("{e}"))),
            }
            set_busy.set(false);
        });
    };

    view! {
        <li class="flex items-center justify-between gap-3 px-3 py-2 rounded hover:bg-[var(--color-surface-hover)]">
            <div class="min-w-0 flex-1">
                <div class="text-sm truncate">{title}</div>
                <div class="faint text-xs truncate">{key}</div>
                {move || error.get().map(|e| view! {
                    <div class="log-line-error text-xs mt-1">{e}</div>
                })}
            </div>
            <div class="flex items-center gap-2 shrink-0">
                {if already_imported {
                    view! { <StatusPill label="Imported".to_string() kind=Status::Ok /> }.into_any()
                } else {
                    view! { <StatusPill label="New".to_string() kind=Status::Stale /> }.into_any()
                }}
                <button
                    type="button"
                    class="btn btn-ghost btn-sm"
                    disabled=move || busy.get() || already_imported
                    on:click=do_import
                >
                    {move || if busy.get() { "Importing…" } else if already_imported { "Imported" } else { "Import" }}
                </button>
            </div>
        </li>
    }
}

async fn upload_file(file: web_sys::File) -> Result<SourceDocumentDto, String> {
    use gloo_net::http::Request;

    let form = web_sys::FormData::new().map_err(|e| format!("could not build FormData: {e:#?}"))?;
    form.append_with_blob_and_filename("file", &file, &file.name())
        .map_err(|e| format!("could not append file to FormData: {e:#?}"))?;

    let response = Request::post("/api/source_documents/upload")
        .body(form)
        .map_err(|e| format!("build request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("upload failed: {e}"))?;

    if !response.ok() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("upload failed ({status}): {body}"));
    }

    response
        .json::<SourceDocumentDto>()
        .await
        .map_err(|e| format!("parse response: {e}"))
}
