use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use uuid::Uuid;
use wasm_bindgen::JsCast;

use crate::server_functions::configuration::{get_chunking_configurations, get_index_profiles};
use crate::server_functions::connector::list_connectors;
use crate::server_functions::source_document::{import_source_document_from_url, request_indexing};
use crate::shared::contracts::{ConnectorDto, IndexProfileDto, SourceDocumentDto};
use crate::shared::ChunkingConfig;
use crate::ui::components::primitives::{Dialog, Help};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Upload,
    Url,
    Connector,
}

#[derive(Clone)]
struct IndexContext {
    profiles: Vec<IndexProfileDto>,
    default_chunk: Option<ChunkingConfig>,
}

impl IndexContext {
    fn can_index(&self) -> bool {
        !self.profiles.is_empty() && self.default_chunk.is_some()
    }
}

#[component]
pub fn AddDialog(#[prop(into)] open: Signal<bool>, on_close: Callback<()>) -> impl IntoView {
    let (tab, set_tab) = signal(Tab::Upload);
    let selected = RwSignal::new(Vec::<Uuid>::new());

    let profiles = Resource::new(
        || (),
        |_| async move { get_index_profiles().await.unwrap_or_default() },
    );
    let chunking = Resource::new(
        || (),
        |_| async move { get_chunking_configurations().await.unwrap_or_default() },
    );
    let connectors = Resource::new(
        || (),
        |_| async move { list_connectors().await.unwrap_or_default() },
    );

    Effect::new(move |_| {
        if let Some(list) = profiles.get() {
            if selected.get_untracked().is_empty() {
                if let Some(p) = list.iter().find(|p| p.is_default).or_else(|| list.first()) {
                    selected.set(vec![p.index_profile_id]);
                }
            }
        }
    });

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

    let connector_close = on_close;
    let close_for_connector = Callback::new(move |_| connector_close.run(()));

    view! {
        <Dialog
            open=open
            title="Add documents"
            subtitle="Bring content into your corpus from a file, a URL, or a connector. Indexed by default.".to_string()
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

            <Transition fallback=|| view! { <p class="text-sm muted py-4">"Loading…"</p> }>
                {move || {
                    let ctx = IndexContext {
                        profiles: profiles.get().unwrap_or_default(),
                        default_chunk: chunking
                            .get()
                            .unwrap_or_default()
                            .into_iter()
                            .find(|c| c.is_default)
                            .map(|c| c.config),
                    };
                    let connector_list = connectors.get().unwrap_or_default();
                    match tab.get() {
                        Tab::Upload => view! {
                            <UploadPane ctx=ctx.clone() selected=selected on_imported=handle_imported />
                        }.into_any(),
                        Tab::Url => view! {
                            <UrlPane ctx=ctx.clone() selected=selected on_imported=handle_imported />
                        }.into_any(),
                        Tab::Connector => view! {
                            <ConnectorPane connectors=connector_list on_navigate=close_for_connector />
                        }.into_any(),
                    }
                }}
            </Transition>
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
fn IndexTargets(
    profiles: Vec<IndexProfileDto>,
    has_default_chunk: bool,
    selected: RwSignal<Vec<Uuid>>,
) -> impl IntoView {
    if profiles.is_empty() {
        return view! {
            <div class="text-xs muted flex items-center gap-2 py-1">
                "No index profile yet — "
                <A href="/pipeline/profiles">"set one up →"</A>
            </div>
        }
        .into_any();
    }
    if !has_default_chunk {
        return view! {
            <div class="text-xs muted flex items-center gap-2 py-1">
                "No default chunking configuration — "
                <A href="/pipeline/chunking">"set one up →"</A>
            </div>
        }
        .into_any();
    }
    if let [only] = profiles.as_slice() {
        let name = only.name.clone();
        return view! {
            <div class="text-xs muted py-1">
                "Will index into "<span class="text-text">{name}</span>"."
            </div>
        }
        .into_any();
    }

    view! {
        <div class="flex flex-col gap-1.5">
            <div class="eyebrow">"Index into"</div>
            <div class="flex flex-col gap-1">
                {profiles.into_iter().map(|p| {
                    let id = p.index_profile_id;
                    let name = p.name.clone();
                    let dims = p.dimensions;
                    let is_default = p.is_default;
                    view! {
                        <label class="flex items-center gap-2 text-sm">
                            <input
                                type="checkbox"
                                prop:checked=move || selected.with(|s| s.contains(&id))
                                on:change=move |_| selected.update(|s| {
                                    if let Some(pos) = s.iter().position(|x| x == &id) {
                                        s.remove(pos);
                                    } else {
                                        s.push(id);
                                    }
                                })
                            />
                            <span class="text-text">{name}</span>
                            {is_default.then(|| view! { <span class="pill pill-neutral text-xs">"default"</span> })}
                            <span class="faint text-xs">{format!("{dims}d")}</span>
                        </label>
                    }
                }).collect_view()}
            </div>
        </div>
    }
    .into_any()
}

async fn run_index(slug: String, targets: Vec<Uuid>, chunk: ChunkingConfig) -> Result<(), String> {
    for profile_id in targets {
        request_indexing(slug.clone(), profile_id, chunk, true)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[component]
fn UploadPane(
    ctx: IndexContext,
    selected: RwSignal<Vec<Uuid>>,
    on_imported: Callback<SourceDocumentDto>,
) -> impl IntoView {
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (selected_name, set_selected_name) = signal::<Option<String>>(None);
    let (is_dragging, set_is_dragging) = signal(false);
    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();

    let can_index = ctx.can_index();
    let default_chunk = StoredValue::new(ctx.default_chunk);
    let profiles = ctx.profiles.clone();

    let run = move |file: web_sys::File, index_after: bool| {
        let targets = selected.get_untracked();
        let chunk = default_chunk.get_value();
        set_selected_name.set(Some(file.name()));
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match upload_file(file).await {
                Ok(dto) => {
                    if index_after {
                        if let Some(chunk) = chunk {
                            if let Err(e) =
                                run_index(dto.source_ref_key.clone(), targets, chunk).await
                            {
                                set_error.set(Some(format!("Added, but indexing failed: {e}")));
                                set_busy.set(false);
                                return;
                            }
                        }
                    }
                    on_imported.run(dto);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_busy.set(false);
        });
    };

    let pick_file = move || -> Option<web_sys::File> {
        let input = file_input.get_untracked()?;
        let files = input.files()?;
        if files.length() == 0 {
            set_error.set(Some("Choose a file first.".into()));
            return None;
        }
        files.item(0)
    };

    let add_and_index = move |_| {
        if busy.get_untracked() {
            return;
        }
        if let Some(file) = pick_file() {
            run(file, true);
        }
    };
    let add_only = move |_| {
        if busy.get_untracked() {
            return;
        }
        if let Some(file) = pick_file() {
            run(file, false);
        }
    };

    let on_dragover = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        if !is_dragging.get_untracked() {
            set_is_dragging.set(true);
        }
    };
    let on_dragleave = move |_ev: leptos::ev::DragEvent| set_is_dragging.set(false);
    let on_drop = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        set_is_dragging.set(false);
        if busy.get_untracked() {
            return;
        }
        let Some(file) = ev
            .data_transfer()
            .and_then(|dt| dt.files())
            .filter(|f| f.length() > 0)
            .and_then(|f| f.item(0))
        else {
            return;
        };
        run(file, true);
    };

    view! {
        <div class="flex flex-col gap-3">
            <label
                class="upload-dropzone"
                class:is-dragging=is_dragging
                on:dragover=on_dragover
                on:dragleave=on_dragleave
                on:drop=on_drop
            >
                <div class="upload-dropzone-icon">"⤓"</div>
                <div class="upload-dropzone-primary">
                    {move || if busy.get() { "Uploading…".to_string() } else { "Drop a file here, or click to choose".to_string() }}
                </div>
                <div class="upload-dropzone-secondary">".md, .markdown, .txt, .html, .pdf"</div>
                {move || selected_name.get().map(|n| view! {
                    <div class="upload-dropzone-selected">{format!("Selected: {n}")}</div>
                })}
                <input
                    type="file"
                    accept=".md,.markdown,.txt,.html,.htm,.pdf,text/markdown,text/plain,text/html,application/pdf"
                    class="hidden"
                    node_ref=file_input
                    on:change=move |ev| {
                        let name = ev.target()
                            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                            .and_then(|el| el.files())
                            .and_then(|f| f.item(0))
                            .map(|f| f.name());
                        set_selected_name.set(name);
                    }
                />
            </label>

            <IndexTargets profiles=profiles has_default_chunk=can_index selected=selected />

            {move || error.get().map(|e| view! { <div class="log-line-error text-sm">{e}</div> })}

            <div class="flex justify-end gap-2">
                <button type="button" class="btn" disabled=busy on:click=add_only>
                    "Add only"
                </button>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=move || busy.get() || !can_index
                    on:click=add_and_index
                >
                    {move || if busy.get() { "Working…" } else { "Add & index" }}
                </button>
            </div>
        </div>
    }
}

#[component]
fn UrlPane(
    ctx: IndexContext,
    selected: RwSignal<Vec<Uuid>>,
    on_imported: Callback<SourceDocumentDto>,
) -> impl IntoView {
    let (url, set_url) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);

    let can_index = ctx.can_index();
    let default_chunk = StoredValue::new(ctx.default_chunk);
    let profiles = ctx.profiles.clone();

    let run = move |index_after: bool| {
        if busy.get_untracked() {
            return;
        }
        let value = url.get_untracked().trim().to_string();
        if value.is_empty() {
            set_error.set(Some("Enter a URL.".into()));
            return;
        }
        let targets = selected.get_untracked();
        let chunk = default_chunk.get_value();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match import_source_document_from_url(value).await {
                Ok(dto) => {
                    if index_after {
                        if let Some(chunk) = chunk {
                            if let Err(e) =
                                run_index(dto.source_ref_key.clone(), targets, chunk).await
                            {
                                set_error.set(Some(format!("Added, but indexing failed: {e}")));
                                set_busy.set(false);
                                return;
                            }
                        }
                    }
                    on_imported.run(dto);
                }
                Err(e) => set_error.set(Some(format!("{e}"))),
            }
            set_busy.set(false);
        });
    };

    let add_and_index = move |_| run(true);
    let add_only = move |_| run(false);

    view! {
        <div class="flex flex-col gap-3">
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

            <IndexTargets profiles=profiles has_default_chunk=can_index selected=selected />

            {move || error.get().map(|e| view! { <div class="log-line-error text-sm">{e}</div> })}

            <div class="flex justify-end gap-2">
                <button type="button" class="btn" disabled=busy on:click=add_only>
                    "Add only"
                </button>
                <button
                    type="button"
                    class="btn btn-primary"
                    disabled=move || busy.get() || !can_index
                    on:click=add_and_index
                >
                    {move || if busy.get() { "Working…" } else { "Add & index" }}
                </button>
            </div>
        </div>
    }
}

#[component]
fn ConnectorPane(connectors: Vec<ConnectorDto>, on_navigate: Callback<()>) -> impl IntoView {
    if connectors.is_empty() {
        return view! {
            <div class="text-sm muted py-4 flex flex-col gap-2">
                <p>"No connectors configured yet."</p>
                <A href="/connectors" attr:class="btn self-start">"Add a connector →"</A>
            </div>
        }
        .into_any();
    }

    view! {
        <div class="flex flex-col gap-2">
            <p class="text-xs muted">
                "Open a connector to sync, browse discovered items, and pull them into your corpus."
            </p>
            <ul class="flex flex-col gap-1">
                {connectors.into_iter().map(|c| {
                    let href = format!("/connectors/{}", c.connector_id);
                    let name = c.name.clone();
                    let kind = c.config.kind().display_label();
                    view! {
                        <li>
                            <A
                                href=href
                                on:click=move |_| on_navigate.run(())
                                attr:class="flex items-center gap-3 rounded px-2 py-2 hover:bg-[var(--color-surface-2)]"
                            >
                                <span class="faint w-5 text-center" aria-hidden="true">"›"</span>
                                <span class="min-w-0 flex-1">
                                    <span class="block text-sm text-text truncate">{name}</span>
                                    <span class="block text-xs muted">{kind}</span>
                                </span>
                            </A>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </div>
    }
    .into_any()
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
