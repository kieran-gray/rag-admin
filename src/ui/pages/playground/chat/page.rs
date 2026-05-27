use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use uuid::Uuid;

use crate::server_functions::configuration::get_retrieval_profiles;
use crate::shared::contracts::{ChatRequest, MetadataFilterDto, RetrievalProfileDto};
use crate::ui::components::playground::metadata_filters::metadata_filters_from_signal;
use crate::ui::components::playground::{
    use_playground_context, QueryInput, RequestInspector, RetrievalControls, RetrievalProfilePicker,
};
use crate::ui::components::primitives::{EmptyState, PageHeader, Surface};

use super::sources_list::SourcesList;
use super::streaming::{cancel_active_stream, run_stream, ChatTurn, StreamSlot, TurnStatus};
use super::turn_view::ChatTranscript;

#[component]
pub fn ChatPage() -> impl IntoView {
    let retrieval_profiles = Resource::new(
        || (),
        |_| async move { get_retrieval_profiles().await.unwrap_or_default() },
    );

    view! {
        <div>
            <PageHeader
                title="Chat"
                subtitle="Ask a question. Retrieves top-K chunks, then prompts the retrieval profile's generation model with the same format used in production.".to_string()
            />
            <Transition fallback=|| view! { <Surface><p class="muted">"Loading…"</p></Surface> }>
                {move || retrieval_profiles.get().map(|profiles| {
                    if profiles.is_empty() {
                        view! {
                            <Surface>
                                <EmptyState
                                    title="No retrieval profiles configured"
                                    body="Create a retrieval profile on the Profiles page before chatting.".to_string()
                                />
                            </Surface>
                        }.into_any()
                    } else {
                        view! { <ChatBody retrieval_profiles=profiles /> }.into_any()
                    }
                })}
            </Transition>
        </div>
    }
}

#[component]
fn ChatBody(retrieval_profiles: Vec<RetrievalProfileDto>) -> impl IntoView {
    let retrieval_profiles_stored = StoredValue::new(retrieval_profiles.clone());
    let playground = use_playground_context();
    let query_params = use_query_map();
    let url_question = query_params.with(|q| q.get("q").unwrap_or_default().to_string());
    let url_profile_param = query_params.with(|q| q.get("profile").unwrap_or_default().to_string());

    let initial_question = if url_question.is_empty() {
        playground.last_query.get_untracked()
    } else {
        url_question
    };
    let url_profile_uuid = Uuid::parse_str(&url_profile_param).ok();
    let initial_retrieval_profile = url_profile_uuid
        .filter(|id| {
            retrieval_profiles
                .iter()
                .any(|p| p.retrieval_profile_id == *id)
        })
        .or_else(|| {
            playground.last_profile.get_untracked().filter(|id| {
                retrieval_profiles
                    .iter()
                    .any(|p| p.retrieval_profile_id == *id)
            })
        })
        .or_else(|| retrieval_profiles.first().map(|p| p.retrieval_profile_id))
        .unwrap_or_default();

    let question = RwSignal::new(initial_question);
    let (retrieval_profile_id, set_retrieval_profile_id) = signal(initial_retrieval_profile);
    let top_k = RwSignal::new(8u32);
    let min_score = RwSignal::new(0.4f32);
    let filters = RwSignal::new(Vec::<MetadataFilterDto>::new());
    let busy = RwSignal::new(false);
    let turns = RwSignal::new(Vec::<ChatTurn>::new());
    let selected_turn = RwSignal::new(None::<Uuid>);
    let active_stream: StoredValue<StreamSlot> = StoredValue::new(None);

    let submit_question = move |question_text: String| {
        if busy.get_untracked() || question_text.trim().is_empty() {
            return;
        }
        cancel_active_stream(active_stream);

        let pid = retrieval_profile_id.get_untracked();
        playground.last_query.set(question_text.clone());
        playground.last_profile.set(Some(pid));
        playground.last_filters.set(filters.get_untracked());
        let req = ChatRequest {
            retrieval_profile_id: pid,
            query: question_text.clone(),
            top_k: top_k.get_untracked(),
            min_score: min_score.get_untracked(),
            metadata_filters: metadata_filters_from_signal(filters),
        };
        let request_json = serde_json::to_string_pretty(&req).unwrap_or_default();
        let turn_id = Uuid::new_v4();

        busy.set(true);
        question.set(String::new());
        selected_turn.set(Some(turn_id));
        turns.update(|t| {
            t.push(ChatTurn {
                turn_id,
                question: question_text,
                status: TurnStatus::Pending,
                meta: None,
                answer: String::new(),
                timings: None,
                error: None,
                request_json,
            });
        });

        run_stream(&req, turn_id, turns, busy, active_stream);
    };

    let submit = Callback::new(move |()| {
        submit_question(question.get_untracked());
    });

    let stop = Callback::new(move |()| {
        cancel_active_stream(active_stream);
        turns.update(|t| {
            if let Some(last) = t.last_mut() {
                if matches!(last.status, TurnStatus::Pending | TurnStatus::Streaming) {
                    last.status = TurnStatus::Aborted;
                }
            }
        });
        busy.set(false);
    });

    let clear_history = move |_| {
        cancel_active_stream(active_stream);
        turns.set(Vec::new());
        selected_turn.set(None);
        busy.set(false);
    };

    let edit_turn = move |turn_id: Uuid| {
        cancel_active_stream(active_stream);
        let q = turns.with(|t| {
            t.iter()
                .find(|x| x.turn_id == turn_id)
                .map(|x| x.question.clone())
        });
        if let Some(q) = q {
            question.set(q);
            turns.update(|t| {
                if let Some(idx) = t.iter().position(|x| x.turn_id == turn_id) {
                    t.truncate(idx);
                }
            });
            selected_turn.set(None);
            busy.set(false);
        }
    };

    let regenerate_turn = move |turn_id: Uuid| {
        let q = turns.with(|t| {
            t.iter()
                .find(|x| x.turn_id == turn_id)
                .map(|x| x.question.clone())
        });
        if let Some(q) = q {
            turns.update(|t| {
                if let Some(idx) = t.iter().position(|x| x.turn_id == turn_id) {
                    t.remove(idx);
                }
            });
            submit_question(q);
        }
    };

    let select_turn = move |turn_id: Uuid| {
        selected_turn.set(Some(turn_id));
    };

    let last_request_body = Signal::derive(move || {
        turns.with(|t| t.last().map(|t| t.request_json.clone()).unwrap_or_default())
    });

    let focused_hits = Signal::derive(move || {
        turns.with(|t| {
            let selected = selected_turn.get();
            let turn = selected
                .and_then(|id| t.iter().find(|x| x.turn_id == id))
                .or_else(|| t.last());
            turn.map(|x| (x.turn_id, x.question.clone(), x.hits().to_vec()))
        })
    });

    view! {
        <div class="playground-grid">
            <div class="playground-main">
                <Surface title="Conversation".to_string() actions=Box::new(move || view! {
                    <div class="flex items-center gap-2">
                        <RetrievalProfilePicker
                            profiles=retrieval_profiles_stored.get_value()
                            value=retrieval_profile_id
                            set_value=set_retrieval_profile_id
                        />
                        <button
                            type="button"
                            class="btn btn-sm btn-ghost"
                            disabled=move || busy.get() || turns.with(Vec::is_empty)
                            on:click=clear_history
                        >
                            "Clear"
                        </button>
                    </div>
                }.into_any())>
                    <div class="playground-body">
                        <ChatTranscript
                            turns=turns
                            selected_turn=selected_turn
                            on_select=select_turn
                            on_edit=edit_turn
                            on_regenerate=regenerate_turn
                        />

                        <QueryInput
                            value=question
                            busy=Signal::from(busy)
                            on_submit=submit
                            on_stop=stop
                            placeholder="Ask a question…".to_string()
                            submit_label="Send".to_string()
                            busy_label="Asking…".to_string()
                        />

                        <RetrievalControls top_k=top_k min_score=min_score filters=filters />

                        {move || {
                            let has_request = turns.with(|t| !t.is_empty());
                            has_request.then(|| view! {
                                <RequestInspector body=last_request_body label="Inspect last request".to_string() />
                            })
                        }}
                    </div>
                </Surface>
            </div>

            <aside class="playground-sidebar">
                <Surface title="Sources".to_string()>
                    {move || {
                        match focused_hits.get() {
                            None => view! {
                                <p class="muted text-sm">"Ask a question to see the retrieved chunks here."</p>
                            }.into_any(),
                            Some((_, _, hits)) if hits.is_empty() => view! {
                                <p class="muted text-sm">"No chunks retrieved for the selected turn."</p>
                            }.into_any(),
                            Some((_, question_text, hits)) => view! {
                                <SourcesList question=question_text hits=hits />
                            }.into_any(),
                        }
                    }}
                </Surface>
            </aside>
        </div>
    }
}
