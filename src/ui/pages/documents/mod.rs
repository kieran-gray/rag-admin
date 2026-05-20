pub mod all_view;
pub mod connector_browse;
pub mod redirect_by_id;
pub mod smart_filter;

pub use redirect_by_id::DocumentByIdRedirect;

use leptos::prelude::*;
use leptos_router::hooks::query_signal;
use uuid::Uuid;

use crate::server_functions::connector::list_connectors;
use crate::shared::contracts::{aggregate_type, ConnectorDto};
use crate::ui::components::app::event_bus::use_invalidator;
use crate::ui::components::app::import_dialog::ImportDialog;
use crate::ui::components::primitives::PageHeader;

use self::all_view::AllDocumentsView;
use self::connector_browse::ConnectorBrowseView;

const TAB_QUERY_PARAM: &str = "tab";

#[derive(Debug, Clone, PartialEq, Eq)]
enum TabSelection {
    AllDocuments,
    Connector(Uuid),
}

impl TabSelection {
    fn from_query(value: Option<&str>) -> Self {
        match value.and_then(parse_connector_token) {
            Some(id) => TabSelection::Connector(id),
            None => TabSelection::AllDocuments,
        }
    }

    fn to_query(&self) -> Option<String> {
        match self {
            TabSelection::AllDocuments => None,
            TabSelection::Connector(id) => Some(format!("connector:{id}")),
        }
    }
}

fn parse_connector_token(value: &str) -> Option<Uuid> {
    value
        .strip_prefix("connector:")
        .and_then(|s| Uuid::parse_str(s).ok())
}

#[component]
pub fn DocumentsPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| e.from_any(&[aggregate_type::CONNECTOR]));
    let connectors = Resource::new(
        move || invalidator.get(),
        |_| async move { list_connectors().await.map_err(|e| e.to_string()) },
    );

    let (tab_query, set_tab_query) = query_signal::<String>(TAB_QUERY_PARAM);
    let selected = Memo::new(move |_| TabSelection::from_query(tab_query.get().as_deref()));

    let (dialog_open, set_dialog_open) = signal(false);
    let open_signal: Signal<bool> = dialog_open.into();
    let close_dialog = Callback::new(move |_| set_dialog_open.set(false));
    let open_import = Callback::new(move |_| set_dialog_open.set(true));

    let select_tab = Callback::<TabSelection>::new(move |tab| {
        set_tab_query.set(tab.to_query());
    });

    view! {
        <div>
            <PageHeader
                title="Documents"
                subtitle="Imported documents, plus discovery surfaces for each configured connector.".to_string()
                actions=Box::new(move || view! {
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=move |_| set_dialog_open.set(true)
                    >
                        "+ Import"
                    </button>
                }.into_any())
            />

            <DocumentsTabs
                connectors=connectors
                selected=selected
                on_select=select_tab
            />

            {move || match selected.get() {
                TabSelection::AllDocuments => view! {
                    <AllDocumentsView open_import=open_import />
                }.into_any(),
                TabSelection::Connector(id) => {
                    let signal: Signal<Uuid> = Signal::derive(move || id);
                    view! { <ConnectorBrowseView connector_id=signal /> }.into_any()
                }
            }}

            <ImportDialog open=open_signal on_close=close_dialog />
        </div>
    }
}

#[component]
fn DocumentsTabs(
    connectors: Resource<Result<Vec<ConnectorDto>, String>>,
    selected: Memo<TabSelection>,
    on_select: Callback<TabSelection>,
) -> impl IntoView {
    view! {
        <nav class="border-b border-[var(--color-border)] mb-4 flex gap-1 flex-wrap">
            <TabButton
                label="All documents".to_string()
                is_active=Signal::derive(move || selected.get() == TabSelection::AllDocuments)
                on_click=Callback::new(move |_| on_select.run(TabSelection::AllDocuments))
            />
            <Suspense fallback=|| view! { <span class="px-3 py-2 faint text-xs">"Loading connectors…"</span> }>
                {move || connectors.get().map(|res| match res {
                    Err(_) => view! { <span></span> }.into_any(),
                    Ok(list) => view! {
                        <>
                            {list.into_iter().map(|c| {
                                let id = c.connector_id;
                                let name = c.name.clone();
                                view! {
                                    <TabButton
                                        label=name
                                        is_active=Signal::derive(move || selected.get() == TabSelection::Connector(id))
                                        on_click=Callback::new(move |_| on_select.run(TabSelection::Connector(id)))
                                    />
                                }
                            }).collect_view()}
                        </>
                    }.into_any(),
                })}
            </Suspense>
        </nav>
    }
}

#[component]
fn TabButton(label: String, is_active: Signal<bool>, on_click: Callback<()>) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || format!(
                "px-4 py-2 -mb-px border-b-2 text-sm font-medium transition-colors {}",
                if is_active.get() {
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
