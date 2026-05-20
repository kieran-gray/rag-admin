use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use wasm_bindgen::JsCast;

use crate::server_functions::source_document::{
    import_source_document_from_url, start_indexing_with_defaults,
};
use crate::shared::contracts::SourceDocumentDto;
use crate::ui::components::primitives::{Dialog, Help};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Upload,
    Url,
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
            subtitle="Upload a file or paste a URL. For bulk imports, use a connector's Browse page.".to_string()
            on_close=on_close
        >
            <nav class="border-b border-[var(--color-border)] mb-4 flex gap-1 -mt-1">
                <TabButton label="Upload"
                    active=move || tab.get() == Tab::Upload
                    on_click=Callback::new(move |_| set_tab.set(Tab::Upload)) />
                <TabButton label="From URL"
                    active=move || tab.get() == Tab::Url
                    on_click=Callback::new(move |_| set_tab.set(Tab::Url)) />
            </nav>

            {move || match tab.get() {
                Tab::Upload => view! { <UploadPane on_imported=handle_imported /> }.into_any(),
                Tab::Url => view! { <UrlPane on_imported=handle_imported /> }.into_any(),
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
                        ".md, .markdown, .txt, .html, .pdf"
                    </div>
                    {move || selected_name.get().map(|n| view! {
                        <div class="upload-dropzone-selected">{format!("Selected: {n}")}</div>
                    })}
                    <input
                        type="file"
                        accept=".md,.markdown,.txt,.html,.htm,.pdf,text/markdown,text/plain,text/html,application/pdf"
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
