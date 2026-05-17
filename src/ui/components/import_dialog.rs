use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use wasm_bindgen::JsCast;

use crate::contracts::{aggregate_type, DocumentListItemDto, SourceDocumentDto};
use crate::server_functions::source_document::{
    import_source_document_from_url, list_adapter_documents, start_indexing_with_defaults,
};
use crate::ui::components::event_bus::use_invalidator;
use crate::ui::components::primitives::{Dialog, Status, StatusPill};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Upload,
    Url,
    Adapter,
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
            subtitle="Upload a file, paste a URL, or pick from a configured source.".to_string()
            on_close=on_close
        >
            <nav class="border-b border-[var(--color-border)] mb-4 flex gap-1 -mt-1">
                <TabButton label="Upload"
                    active=move || tab.get() == Tab::Upload
                    on_click=Callback::new(move |_| set_tab.set(Tab::Upload)) />
                <TabButton label="From URL"
                    active=move || tab.get() == Tab::Url
                    on_click=Callback::new(move |_| set_tab.set(Tab::Url)) />
                <TabButton label="From source"
                    active=move || tab.get() == Tab::Adapter
                    on_click=Callback::new(move |_| set_tab.set(Tab::Adapter)) />
            </nav>

            {move || match tab.get() {
                Tab::Upload => view! { <UploadPane on_imported=handle_imported /> }.into_any(),
                Tab::Url => view! { <UrlPane on_imported=handle_imported /> }.into_any(),
                Tab::Adapter => view! { <AdapterPane /> }.into_any(),
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
        <label class="flex items-center gap-2 text-sm">
            <input
                type="checkbox"
                prop:checked=index_after
                on:change=move |ev| set_index_after.set(event_target_checked(&ev))
            />
            <span>"Index immediately with default pipeline"</span>
        </label>
    }
}

#[component]
fn UploadPane(on_imported: Callback<SourceDocumentDto>) -> impl IntoView {
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (selected_name, set_selected_name) = signal::<Option<String>>(None);
    let (index_after, set_index_after) = signal(false);
    let file_input: NodeRef<leptos::html::Input> = NodeRef::new();

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
        let auto_index = index_after.get_untracked();

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

    view! {
        <form on:submit=submit class="flex flex-col gap-3">
                <label
                    class="border border-dashed border-[var(--color-border)] rounded-md p-6 flex flex-col items-center justify-center gap-2 cursor-pointer hover:bg-[var(--color-surface-hover)]"
                >
                    <span class="text-sm">"Click to choose a file"</span>
                    <span class="muted text-xs">".md, .markdown, .txt"</span>
                    {move || selected_name.get().map(|n| view! {
                        <span class="text-xs mt-1">{format!("Selected: {n}")}</span>
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
                <span class="muted">"URL"</span>
                <input
                    type="url"
                    class="input"
                    placeholder="https://example.com/article"
                    prop:value=url
                    on:input=move |ev| set_url.set(event_target_value(&ev))
                />
            </label>
            <p class="muted text-xs">"The page is fetched server-side and converted to Markdown before ingestion."</p>

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
fn AdapterPane() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[aggregate_type::SOURCE_DOCUMENT, aggregate_type::INDEXING])
    });
    let docs = Resource::new(
        move || invalidator.get(),
        |_| async move { list_adapter_documents().await },
    );

    view! {
        <Suspense fallback=|| view! { <p class="muted text-sm">"Loading sources…"</p> }>
            {move || docs.get().map(|res| match res {
                Err(e) => view! {
                    <div class="log-line-error text-sm">{format!("Failed to load: {e}")}</div>
                }.into_any(),
                Ok(items) => {
                    let available: Vec<DocumentListItemDto> = items
                        .into_iter()
                        .filter(|d| d.document_id.is_none())
                        .collect();
                    if available.is_empty() {
                        view! {
                            <p class="muted text-sm">
                                "No upstream sources are configured. Set BLOG_URL or register another adapter to enable this tab."
                            </p>
                        }.into_any()
                    } else {
                        view! {
                            <ul class="flex flex-col gap-1">
                                {available.into_iter().map(|d| view! { <AdapterRow doc=d /> }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }
            })}
        </Suspense>
    }
}

#[component]
fn AdapterRow(doc: DocumentListItemDto) -> impl IntoView {
    let href = format!(
        "/documents/{}/{}",
        doc.document_type.to_lowercase(),
        urlencoding::encode(&doc.source_ref_key)
    );
    view! {
        <li class="flex items-center justify-between gap-3 px-3 py-2 rounded hover:bg-[var(--color-surface-hover)]">
            <div class="min-w-0">
                <div class="text-sm truncate">{doc.title.clone()}</div>
                <div class="faint text-xs truncate">{format!("./{}", doc.source_ref_key)}</div>
            </div>
            <div class="flex items-center gap-2 shrink-0">
                <StatusPill label="Not imported".to_string() kind=Status::Stale />
                <a href=href class="btn btn-ghost btn-sm">"Open ›"</a>
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
