use leptos::prelude::*;
use leptos_router::components::A;
use uuid::Uuid;

use crate::server_functions::configuration::{
    get_configuration, get_index_profiles, get_retrieval_profiles,
};
use crate::shared::contracts::{
    aggregate_type, ConfigurationDto, CreateIndexProfileDto, CreateRetrievalProfileDto,
    DeleteIndexProfileDto, DeleteRetrievalProfileDto, IndexProfileCommandDto, IndexProfileDto,
    RetrievalProfileCommandDto, RetrievalProfileDto, UpdateIndexProfileDto,
    UpdateRetrievalProfileDto,
};
use crate::ui::components::primitives::{
    Dialog, EmptyState, InlineStatusMessage, PageHeader, Surface,
};
use crate::ui::pages::configuration::commands::{
    parse_uuid_or_none, run_index_profile_command, run_retrieval_profile_command,
};
use crate::ui::state::event_bus::use_invalidator;

#[derive(Clone)]
enum IndexFormMode {
    Add,
    Edit(IndexProfileDto),
}

#[derive(Clone)]
enum RetrievalFormMode {
    Add,
    Edit(RetrievalProfileDto),
}

#[component]
pub fn ProfilesPage() -> impl IntoView {
    let invalidator = use_invalidator(|e| {
        e.from_any(&[
            aggregate_type::EMBEDDING_MODEL_CATALOG,
            aggregate_type::VECTOR_INDEX_CATALOG,
            aggregate_type::GENERATION_MODEL_CATALOG,
            aggregate_type::INDEX_PROFILE_CATALOG,
        ])
    });
    let (refresh, set_refresh) = signal(0u32);

    let configuration = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_configuration().await.map_err(|e| e.to_string()) },
    );
    let index_profiles = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_index_profiles().await.map_err(|e| e.to_string()) },
    );
    let retrieval_profiles = Resource::new(
        move || (invalidator.get(), refresh.get()),
        |_| async move { get_retrieval_profiles().await.map_err(|e| e.to_string()) },
    );

    let (busy, set_busy) = signal(false);
    let (status, set_status) = signal::<Option<InlineStatusMessage>>(None);
    let (index_form_mode, set_index_form_mode) = signal::<Option<IndexFormMode>>(None);
    let (index_delete_target, set_index_delete_target) = signal::<Option<IndexProfileDto>>(None);
    let (retrieval_form_mode, set_retrieval_form_mode) = signal::<Option<RetrievalFormMode>>(None);
    let (retrieval_delete_target, set_retrieval_delete_target) =
        signal::<Option<RetrievalProfileDto>>(None);

    view! {
        <div>
            <PageHeader
                title="Profiles"
                subtitle="Index profiles pair an embedding model with a vector index. Retrieval profiles bind an index profile to a generation model and query defaults.".to_string()
            />

            <StatusBanner status=status />

            <SectionHeader
                id="index-profiles".to_string()
                title="Index profiles".to_string()
                hint="Ingest writes here; retrieval reads from here.".to_string()
                button_label="+ New index profile".to_string()
                on_new=Callback::new(move |_| set_index_form_mode.set(Some(IndexFormMode::Add)))
            />

            <Transition fallback=|| view! { <p class="muted">"Loading index profiles…"</p> }>
                {move || {
                    let (cfg, list) = match (configuration.get(), index_profiles.get()) {
                        (Some(Ok(c)), Some(Ok(l))) => (c, l),
                        (Some(Err(e)), _) | (_, Some(Err(e))) => {
                            return view! {
                                <Surface>
                                    <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                                </Surface>
                            }.into_any();
                        }
                        _ => return ().into_any(),
                    };

                    view! {
                        <IndexProfileList
                            profiles=list
                            on_edit=Callback::new(move |p: IndexProfileDto| set_index_form_mode.set(Some(IndexFormMode::Edit(p))))
                            on_delete=Callback::new(move |p: IndexProfileDto| set_index_delete_target.set(Some(p)))
                            registry_hint=cfg.embedding_models.is_empty() || cfg.vector_indexes.is_empty()
                            busy=busy
                        />
                    }.into_any()
                }}
            </Transition>

            <SectionHeader
                id="retrieval-profiles".to_string()
                title="Retrieval profiles".to_string()
                hint="Chat, playground, and dataset generation use these.".to_string()
                button_label="+ New retrieval profile".to_string()
                on_new=Callback::new(move |_| set_retrieval_form_mode.set(Some(RetrievalFormMode::Add)))
            />

            <Transition fallback=|| view! { <p class="muted">"Loading retrieval profiles…"</p> }>
                {move || {
                    let (cfg, ips, list) = match (configuration.get(), index_profiles.get(), retrieval_profiles.get()) {
                        (Some(Ok(c)), Some(Ok(i)), Some(Ok(l))) => (c, i, l),
                        (Some(Err(e)), _, _) | (_, Some(Err(e)), _) | (_, _, Some(Err(e))) => {
                            return view! {
                                <Surface>
                                    <div class="log-line-error">{format!("Failed to load: {e}")}</div>
                                </Surface>
                            }.into_any();
                        }
                        _ => return ().into_any(),
                    };

                    view! {
                        <RetrievalProfileList
                            profiles=list
                            on_edit=Callback::new(move |p: RetrievalProfileDto| set_retrieval_form_mode.set(Some(RetrievalFormMode::Edit(p))))
                            on_delete=Callback::new(move |p: RetrievalProfileDto| set_retrieval_delete_target.set(Some(p)))
                            registry_hint=cfg.generation_models.is_empty() || ips.is_empty()
                            busy=busy
                        />
                    }.into_any()
                }}
            </Transition>

            <Transition fallback=|| ()>
                {move || configuration.get().map(|res| match res {
                    Ok(cfg) => view! {
                        <IndexProfileFormDialog
                            config=cfg
                            form_mode=index_form_mode
                            set_form_mode=set_index_form_mode
                            busy=busy
                            set_busy=set_busy
                            set_status=set_status
                            set_refresh=set_refresh
                        />
                    }.into_any(),
                    Err(_) => ().into_any(),
                })}
            </Transition>

            <Transition fallback=|| ()>
                {move || match (configuration.get(), index_profiles.get()) {
                    (Some(Ok(cfg)), Some(Ok(ips))) => view! {
                        <RetrievalProfileFormDialog
                            config=cfg
                            index_profiles=ips
                            form_mode=retrieval_form_mode
                            set_form_mode=set_retrieval_form_mode
                            busy=busy
                            set_busy=set_busy
                            set_status=set_status
                            set_refresh=set_refresh
                        />
                    }.into_any(),
                    _ => ().into_any(),
                }}
            </Transition>

            <IndexDeleteDialog
                target=index_delete_target
                set_target=set_index_delete_target
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />

            <RetrievalDeleteDialog
                target=retrieval_delete_target
                set_target=set_retrieval_delete_target
                busy=busy
                set_busy=set_busy
                set_status=set_status
                set_refresh=set_refresh
            />
        </div>
    }
}

#[component]
fn SectionHeader(
    id: String,
    title: String,
    hint: String,
    button_label: String,
    on_new: Callback<()>,
) -> impl IntoView {
    view! {
        <div id=id class="mt-8 mb-3 flex items-end justify-between gap-3">
            <div>
                <h2 class="section-title">{title}</h2>
                <p class="text-sm muted">{hint}</p>
            </div>
            <button
                type="button"
                class="btn btn-primary"
                on:click=move |_| on_new.run(())
            >
                {button_label}
            </button>
        </div>
    }
}

#[component]
fn StatusBanner(status: ReadSignal<Option<InlineStatusMessage>>) -> impl IntoView {
    view! {
        {move || status.get().map(|m| {
            let cls = if m.ok {
                "surface mb-4 px-4 py-2"
            } else {
                "surface mb-4 px-4 py-2 log-line-error"
            };
            view! { <div class=cls>{m.text}</div> }
        })}
    }
}

#[component]
fn IndexProfileList(
    profiles: Vec<IndexProfileDto>,
    on_edit: Callback<IndexProfileDto>,
    on_delete: Callback<IndexProfileDto>,
    registry_hint: bool,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    if profiles.is_empty() {
        return view! {
            <Surface>
                <EmptyState
                    title="No index profiles yet"
                    body=if registry_hint {
                        "Add at least one embedding model and vector index in the Catalog, then come back to compose them into an index profile.".to_string()
                    } else {
                        "Index profiles bake an embedding model and vector index together for ingestion and retrieval.".to_string()
                    }
                    action=Box::new(|| view! {
                        <A href="/configuration/catalog" attr:class="btn">"Open Catalog"</A>
                    }.into_any())
                />
            </Surface>
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            {profiles.into_iter().map(|p| view! {
                <IndexProfileCard
                    profile=p
                    on_edit=on_edit
                    on_delete=on_delete
                    busy=busy
                />
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn IndexProfileCard(
    profile: IndexProfileDto,
    on_edit: Callback<IndexProfileDto>,
    on_delete: Callback<IndexProfileDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let p_edit = profile.clone();
    let p_delete = profile.clone();
    let name = profile.name.clone();
    let embedding = profile
        .embedding_model_name
        .clone()
        .unwrap_or_else(|| short_uuid(profile.embedding_model_id));
    let index = profile
        .vector_index_name
        .clone()
        .unwrap_or_else(|| short_uuid(profile.vector_index_id));

    let is_default = profile.is_default;
    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <h3 class="section-title flex items-center gap-2">
                    {name}
                    {is_default.then(|| view! {
                        <span class="pill pill-accent text-xs">"default"</span>
                    })}
                </h3>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{format!("embed · {embedding}")}</span>
                    <span class="pill pill-neutral">{format!("index · {index}")}</span>
                </div>
            </div>
            <div class="flex gap-2 shrink-0">
                <button type="button" class="btn" disabled=busy on:click=move |_| on_edit.run(p_edit.clone())>"Edit"</button>
                <button type="button" class="btn" disabled=busy on:click=move |_| on_delete.run(p_delete.clone())>"Delete"</button>
            </div>
        </div>
    }
}

#[component]
fn RetrievalProfileList(
    profiles: Vec<RetrievalProfileDto>,
    on_edit: Callback<RetrievalProfileDto>,
    on_delete: Callback<RetrievalProfileDto>,
    registry_hint: bool,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    if profiles.is_empty() {
        return view! {
            <Surface>
                <EmptyState
                    title="No retrieval profiles yet"
                    body=if registry_hint {
                        "Add at least one generation model in the Catalog and one index profile above, then compose them into a retrieval profile.".to_string()
                    } else {
                        "Retrieval profiles bind an index profile to a generation model and retrieval defaults.".to_string()
                    }
                    action=Box::new(|| view! {
                        <A href="#index-profiles" attr:class="btn">"Add an index profile"</A>
                    }.into_any())
                />
            </Surface>
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            {profiles.into_iter().map(|p| view! {
                <RetrievalProfileCard
                    profile=p
                    on_edit=on_edit
                    on_delete=on_delete
                    busy=busy
                />
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn RetrievalProfileCard(
    profile: RetrievalProfileDto,
    on_edit: Callback<RetrievalProfileDto>,
    on_delete: Callback<RetrievalProfileDto>,
    busy: ReadSignal<bool>,
) -> impl IntoView {
    let p_edit = profile.clone();
    let p_delete = profile.clone();
    let name = profile.name.clone();
    let index_profile = profile
        .index_profile_name
        .clone()
        .unwrap_or_else(|| short_uuid(profile.index_profile_id));
    let generation = profile
        .generation_model_name
        .clone()
        .unwrap_or_else(|| short_uuid(profile.generation_model_id));

    let is_default = profile.is_default;
    let top_k = profile.default_top_k;
    let min_score = profile.default_min_score_milli;
    view! {
        <div class="surface p-4 flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div class="space-y-2 min-w-0">
                <h3 class="section-title flex items-center gap-2">
                    {name}
                    {is_default.then(|| view! {
                        <span class="pill pill-accent text-xs">"default"</span>
                    })}
                </h3>
                <div class="flex gap-1.5 flex-wrap text-sm muted">
                    <span class="pill pill-neutral">{format!("index · {index_profile}")}</span>
                    <span class="pill pill-neutral">{format!("gen · {generation}")}</span>
                    <span class="pill pill-neutral">{format!("top_k {top_k}")}</span>
                    <span class="pill pill-neutral">{format!("min_score {min_score}/1000")}</span>
                </div>
            </div>
            <div class="flex gap-2 shrink-0">
                <button type="button" class="btn" disabled=busy on:click=move |_| on_edit.run(p_edit.clone())>"Edit"</button>
                <button type="button" class="btn" disabled=busy on:click=move |_| on_delete.run(p_delete.clone())>"Delete"</button>
            </div>
        </div>
    }
}

#[component]
fn IndexProfileFormDialog(
    config: ConfigurationDto,
    form_mode: ReadSignal<Option<IndexFormMode>>,
    set_form_mode: WriteSignal<Option<IndexFormMode>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let config = StoredValue::new(config);
    let (name, set_name) = signal(String::new());
    let (embedding_id, set_embedding_id) = signal::<Option<Uuid>>(None);
    let (vector_index_id, set_vector_index_id) = signal::<Option<Uuid>>(None);
    let (is_default, set_is_default) = signal(false);
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form_mode.get() {
            None => {}
            Some(IndexFormMode::Add) => {
                set_name.set(String::new());
                set_embedding_id.set(
                    config.with_value(|c| c.embedding_models.first().map(|m| m.embedding_model_id)),
                );
                set_vector_index_id
                    .set(config.with_value(|c| c.vector_indexes.first().map(|i| i.index_id)));
                set_is_default.set(false);
            }
            Some(IndexFormMode::Edit(p)) => {
                set_name.set(p.name);
                set_embedding_id.set(Some(p.embedding_model_id));
                set_vector_index_id.set(Some(p.vector_index_id));
                set_is_default.set(p.is_default);
            }
        }
    });

    let close = Callback::new(move |_| {
        set_form_mode.set(None);
        set_dialog_error.set(None);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (Some(emb), Some(idx)) = (embedding_id.get(), vector_index_id.get()) else {
            set_dialog_error.set(Some("Pick an embedding model and vector index.".into()));
            return;
        };
        let name_val = name.get().trim().to_string();
        if name_val.is_empty() {
            set_dialog_error.set(Some("Index profile name is required.".into()));
            return;
        }
        let default_flag = is_default.get();
        let command = match form_mode.get() {
            Some(IndexFormMode::Add) => {
                IndexProfileCommandDto::CreateIndexProfile(CreateIndexProfileDto {
                    name: name_val,
                    embedding_model_id: emb,
                    vector_index_id: idx,
                    is_default: default_flag,
                })
            }
            Some(IndexFormMode::Edit(p)) => {
                IndexProfileCommandDto::UpdateIndexProfile(UpdateIndexProfileDto {
                    index_profile_id: p.index_profile_id,
                    name: name_val,
                    embedding_model_id: emb,
                    vector_index_id: idx,
                    is_default: default_flag,
                })
            }
            None => return,
        };
        run_index_profile_command(
            command,
            "Index profile saved",
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
                Some(IndexFormMode::Edit(_)) => "Edit index profile".to_string(),
                _ => "New index profile".to_string(),
            })
            subtitle="Embedding model and vector index must share the same dimension.".to_string()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|msg| view! {
                    <div class="log-line-error text-sm">{msg}</div>
                })}

                <LabelledInput
                    label="Name".to_string()
                    hint="e.g. production-cf, local-ollama".to_string()
                    value=name
                    set_value=set_name
                />

                <LabelledSelect
                    label="Embedding model".to_string()
                    placeholder="— select embedding model —".to_string()
                    value=embedding_id
                    set_value=set_embedding_id
                    options=Memo::new(move |_| config.with_value(|c| {
                        c.embedding_models
                            .iter()
                            .map(|m| (
                                m.embedding_model_id,
                                format!("{} · {} · {}d", m.kind.display_label(), m.model, m.dimensions),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <LabelledSelect
                    label="Vector index".to_string()
                    placeholder="— select vector index —".to_string()
                    value=vector_index_id
                    set_value=set_vector_index_id
                    options=Memo::new(move |_| config.with_value(|c| {
                        c.vector_indexes
                            .iter()
                            .map(|i| (
                                i.index_id,
                                format!("{} · {} · {}d", i.kind.display_label(), i.name, i.dimensions),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <label class="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        prop:checked=is_default
                        on:change=move |ev| set_is_default.set(event_target_checked(&ev))
                    />
                    <span>"Use as default index profile (one-click indexing uses this)"</span>
                </label>

                <div class="flex justify-end gap-2 pt-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="submit" class="btn btn-primary" disabled=busy>
                        {move || if busy.get() { "Saving…" } else { "Save index profile" }}
                    </button>
                </div>
            </form>
        </Dialog>
    }
}

#[component]
fn RetrievalProfileFormDialog(
    config: ConfigurationDto,
    index_profiles: Vec<IndexProfileDto>,
    form_mode: ReadSignal<Option<RetrievalFormMode>>,
    set_form_mode: WriteSignal<Option<RetrievalFormMode>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let config = StoredValue::new(config);
    let index_profiles = StoredValue::new(index_profiles);
    let (name, set_name) = signal(String::new());
    let (index_profile_id, set_index_profile_id) = signal::<Option<Uuid>>(None);
    let (generation_id, set_generation_id) = signal::<Option<Uuid>>(None);
    let (reranker_id, set_reranker_id) = signal::<Option<Uuid>>(None);
    let (top_k, set_top_k) = signal(5u32);
    let (min_score_milli, set_min_score_milli) = signal(300u32);
    let (is_default, set_is_default) = signal(false);
    let (dialog_error, set_dialog_error) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        set_dialog_error.set(None);
        match form_mode.get() {
            None => {}
            Some(RetrievalFormMode::Add) => {
                set_name.set(String::new());
                set_index_profile_id.set(
                    index_profiles.with_value(|ips| ips.first().map(|ip| ip.index_profile_id)),
                );
                set_generation_id
                    .set(config.with_value(|c| {
                        c.generation_models.first().map(|m| m.generation_model_id)
                    }));
                set_reranker_id.set(None);
                set_top_k.set(5);
                set_min_score_milli.set(300);
                set_is_default.set(false);
            }
            Some(RetrievalFormMode::Edit(p)) => {
                set_name.set(p.name);
                set_index_profile_id.set(Some(p.index_profile_id));
                set_generation_id.set(Some(p.generation_model_id));
                set_reranker_id.set(p.reranker_model_id);
                set_top_k.set(p.default_top_k);
                set_min_score_milli.set(p.default_min_score_milli);
                set_is_default.set(p.is_default);
            }
        }
    });

    let close = Callback::new(move |_| {
        set_form_mode.set(None);
        set_dialog_error.set(None);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (Some(ip), Some(gen)) = (index_profile_id.get(), generation_id.get()) else {
            set_dialog_error.set(Some("Pick an index profile and generation model.".into()));
            return;
        };
        let name_val = name.get().trim().to_string();
        if name_val.is_empty() {
            set_dialog_error.set(Some("Retrieval profile name is required.".into()));
            return;
        }
        let top_k_val = top_k.get();
        if top_k_val == 0 {
            set_dialog_error.set(Some("top_k must be greater than zero.".into()));
            return;
        }
        let min_score_val = min_score_milli.get();
        if min_score_val > 1000 {
            set_dialog_error.set(Some("min_score must be between 0 and 1000.".into()));
            return;
        }
        let default_flag = is_default.get();
        let command = match form_mode.get() {
            Some(RetrievalFormMode::Add) => {
                RetrievalProfileCommandDto::CreateRetrievalProfile(CreateRetrievalProfileDto {
                    name: name_val,
                    index_profile_id: ip,
                    generation_model_id: gen,
                    reranker_model_id: reranker_id.get(),
                    default_top_k: top_k_val,
                    default_min_score_milli: min_score_val,
                    is_default: default_flag,
                })
            }
            Some(RetrievalFormMode::Edit(p)) => {
                RetrievalProfileCommandDto::UpdateRetrievalProfile(UpdateRetrievalProfileDto {
                    retrieval_profile_id: p.retrieval_profile_id,
                    name: name_val,
                    index_profile_id: ip,
                    generation_model_id: gen,
                    reranker_model_id: reranker_id.get(),
                    default_top_k: top_k_val,
                    default_min_score_milli: min_score_val,
                    is_default: default_flag,
                })
            }
            None => return,
        };
        run_retrieval_profile_command(
            command,
            "Retrieval profile saved",
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
                Some(RetrievalFormMode::Edit(_)) => "Edit retrieval profile".to_string(),
                _ => "New retrieval profile".to_string(),
            })
            subtitle="Select the index profile to query, the model to generate answers with, and the default retrieval knobs.".to_string()
            on_close=close
        >
            <form on:submit=submit class="space-y-4">
                {move || dialog_error.get().map(|msg| view! {
                    <div class="log-line-error text-sm">{msg}</div>
                })}

                <LabelledInput
                    label="Name".to_string()
                    hint="e.g. production-claude, local-ollama".to_string()
                    value=name
                    set_value=set_name
                />

                <LabelledSelect
                    label="Index profile".to_string()
                    placeholder="— select index profile —".to_string()
                    value=index_profile_id
                    set_value=set_index_profile_id
                    options=Memo::new(move |_| index_profiles.with_value(|ips| {
                        ips.iter()
                            .map(|ip| (
                                ip.index_profile_id,
                                format!(
                                    "{} · {}d",
                                    ip.name,
                                    ip.dimensions,
                                ),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <LabelledSelect
                    label="Generation model".to_string()
                    placeholder="— select generation model —".to_string()
                    value=generation_id
                    set_value=set_generation_id
                    options=Memo::new(move |_| config.with_value(|c| {
                        c.generation_models
                            .iter()
                            .map(|m| (
                                m.generation_model_id,
                                format!("{} · {}", m.kind.display_label(), m.model),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <LabelledSelect
                    label="Reranker (optional)".to_string()
                    placeholder="— none —".to_string()
                    value=reranker_id
                    set_value=set_reranker_id
                    options=Memo::new(move |_| config.with_value(|c| {
                        c.reranker_models
                            .iter()
                            .map(|m| (
                                m.reranker_model_id,
                                format!("{} · {}", m.kind.display_label(), m.model),
                            ))
                            .collect::<Vec<_>>()
                    }))
                />

                <LabelledNumber
                    label="Default top_k".to_string()
                    hint="number of chunks to retrieve per query".to_string()
                    value=top_k
                    set_value=set_top_k
                />

                <LabelledNumber
                    label="Default min_score (milli)".to_string()
                    hint="0–1000; chunks below this are dropped".to_string()
                    value=min_score_milli
                    set_value=set_min_score_milli
                />

                <label class="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        prop:checked=is_default
                        on:change=move |ev| set_is_default.set(event_target_checked(&ev))
                    />
                    <span>"Use as default retrieval profile (chat / playground / dataset gen use this)"</span>
                </label>

                <div class="flex justify-end gap-2 pt-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>
                        "Cancel"
                    </button>
                    <button type="submit" class="btn btn-primary" disabled=busy>
                        {move || if busy.get() { "Saving…" } else { "Save retrieval profile" }}
                    </button>
                </div>
            </form>
        </Dialog>
    }
}

#[component]
fn IndexDeleteDialog(
    target: ReadSignal<Option<IndexProfileDto>>,
    set_target: WriteSignal<Option<IndexProfileDto>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(p) = target.get_untracked() else {
            return;
        };
        run_index_profile_command(
            IndexProfileCommandDto::DeleteIndexProfile(DeleteIndexProfileDto {
                index_profile_id: p.index_profile_id,
            }),
            "Index profile deleted",
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
            title="Delete index profile".to_string()
            subtitle="The catalog entries this profile references are not affected.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised p-3 rounded">
                    <span class="muted text-sm">"Index profile"</span>
                    <div class="text-text">{move || target.get().map(|p| p.name).unwrap_or_default()}</div>
                </div>
                <div class="flex justify-end gap-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>"Cancel"</button>
                    <button type="button" class="btn btn-primary" disabled=busy on:click=confirm>
                        {move || if busy.get() { "Deleting…" } else { "Delete index profile" }}
                    </button>
                </div>
            </div>
        </Dialog>
    }
}

#[component]
fn RetrievalDeleteDialog(
    target: ReadSignal<Option<RetrievalProfileDto>>,
    set_target: WriteSignal<Option<RetrievalProfileDto>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_status: WriteSignal<Option<InlineStatusMessage>>,
    set_refresh: WriteSignal<u32>,
) -> impl IntoView {
    let close = Callback::new(move |_| set_target.set(None));

    let confirm = move |_| {
        let Some(p) = target.get_untracked() else {
            return;
        };
        run_retrieval_profile_command(
            RetrievalProfileCommandDto::DeleteRetrievalProfile(DeleteRetrievalProfileDto {
                retrieval_profile_id: p.retrieval_profile_id,
            }),
            "Retrieval profile deleted",
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
            title="Delete retrieval profile".to_string()
            subtitle="The catalog entries this profile references are not affected.".to_string()
            on_close=close
        >
            <div class="space-y-4">
                <div class="surface-raised p-3 rounded">
                    <span class="muted text-sm">"Retrieval profile"</span>
                    <div class="text-text">{move || target.get().map(|p| p.name).unwrap_or_default()}</div>
                </div>
                <div class="flex justify-end gap-2">
                    <button type="button" class="btn" disabled=busy on:click=move |_| close.run(())>"Cancel"</button>
                    <button type="button" class="btn btn-primary" disabled=busy on:click=confirm>
                        {move || if busy.get() { "Deleting…" } else { "Delete retrieval profile" }}
                    </button>
                </div>
            </div>
        </Dialog>
    }
}

#[component]
fn LabelledInput(
    label: String,
    hint: String,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                prop:value=move || value.get()
                on:input=move |e| set_value.set(event_target_value(&e))
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

#[component]
fn LabelledNumber(
    label: String,
    hint: String,
    value: ReadSignal<u32>,
    set_value: WriteSignal<u32>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <input
                class="input"
                r#type="number"
                prop:value=move || value.get().to_string()
                on:input=move |e| {
                    if let Ok(v) = event_target_value(&e).parse() {
                        set_value.set(v);
                    }
                }
            />
            <span class="text-xs faint">{hint}</span>
        </label>
    }
}

#[component]
fn LabelledSelect(
    label: String,
    placeholder: String,
    value: ReadSignal<Option<Uuid>>,
    set_value: WriteSignal<Option<Uuid>>,
    options: Memo<Vec<(Uuid, String)>>,
) -> impl IntoView {
    view! {
        <label class="block space-y-1.5">
            <span class="eyebrow">{label}</span>
            <select
                class="input"
                on:change=move |e| set_value.set(parse_uuid_or_none(&event_target_value(&e)))
            >
                <option value="">{placeholder.clone()}</option>
                {move || options.get().into_iter().map(|(id, lab)| {
                    let selected = value.get() == Some(id);
                    view! { <option value=id.to_string() selected=selected>{lab}</option> }
                }).collect_view()}
            </select>
        </label>
    }
}

fn short_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}
