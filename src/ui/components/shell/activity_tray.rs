use leptos::prelude::*;
use uuid::Uuid;

use crate::contracts::{ActivityJobDto, ActivityKind, ActivityStatus};
use crate::ui::components::activity::{
    clamp_tray_height, dismiss_job, set_tray_open, use_activity_state,
};
use crate::ui::components::log_stream::LogStream;

#[component]
pub fn ActivityTray() -> impl IntoView {
    let state = use_activity_state();
    let open = state.open;
    let selected = state.selected;
    let height = state.height;

    let jobs = Signal::derive(move || state.ordered_jobs());
    let running_jobs = Signal::derive(move || state.running_jobs());

    #[cfg(feature = "hydrate")]
    {
        self::hydrate::install_keyboard_shortcut();
    }

    let effective_selected = Memo::new(move |_| {
        let list = jobs.get();
        if list.is_empty() {
            return None;
        }
        let cur = selected.get();
        if let Some(id) = cur {
            if list.iter().any(|j| j.stream_id == id) {
                return Some(id);
            }
        }
        running_jobs
            .get()
            .first()
            .map(|j| j.stream_id)
            .or_else(|| list.first().map(|j| j.stream_id))
    });

    let active_job = Memo::new(move |_| {
        let id = effective_selected.get()?;
        jobs.get().into_iter().find(|j| j.stream_id == id)
    });

    let stream_url = Signal::derive(move || {
        let job = active_job.get()?;
        job.stream_url
            .clone()
            .or_else(|| Some(format!("/api/job/logs/{}", job.stream_id)))
    });

    let tray_style = move || format!("height: {}px", clamp_tray_height(height.get()));
    let active_signal: Signal<Option<Uuid>> = Signal::derive(move || effective_selected.get());
    let on_select_tab = move |id: Uuid| selected.set(Some(id));

    view! {
        {move || open.get().then(|| view! {
            <aside
                class="activity-tray"
                aria-label="Activity tray"
                style=tray_style
            >
                <ResizeHandle height=height />
                <div class="activity-tray-header">
                    <TabStrip
                        jobs=jobs
                        active_id=active_signal
                        on_select=on_select_tab
                    />
                    <div class="activity-tray-actions">
                        <span class="activity-tray-hint">
                            {move || format_hint(&jobs.get())}
                        </span>
                        <button
                            type="button"
                            class="activity-tray-icon-btn"
                            title="Close tray (Ctrl+`)"
                            aria-label="Close tray"
                            on:click=move |_| set_tray_open(false)
                        >
                            "×"
                        </button>
                    </div>
                </div>
                <div class="activity-tray-body">
                    {move || match active_job.get() {
                        None => view! { <EmptyState /> }.into_any(),
                        Some(job) => view! {
                            <JobDetail job=job url=stream_url />
                        }.into_any(),
                    }}
                </div>
            </aside>
        })}
    }
}

#[component]
fn ResizeHandle(height: RwSignal<f64>) -> impl IntoView {
    view! {
        <div
            class="activity-tray-resize"
            on:mousedown=move |ev| {
                #[cfg(feature = "hydrate")]
                {
                    self::hydrate::begin_resize(ev.client_y(), height);
                }
                #[cfg(not(feature = "hydrate"))]
                {
                    let _ = ev;
                    let _ = height;
                }
                ev.prevent_default();
            }
            title="Drag to resize"
            aria-label="Resize tray"
            role="separator"
        ></div>
    }
}

#[component]
fn TabStrip(
    #[prop(into)] jobs: Signal<Vec<ActivityJobDto>>,
    #[prop(into)] active_id: Signal<Option<Uuid>>,
    on_select: impl Fn(Uuid) + Send + Sync + Clone + 'static,
) -> impl IntoView {
    let on_select = StoredValue::new(on_select);

    view! {
        <div class="activity-tray-tabs" role="tablist">
            {move || {
                let list = jobs.get();
                let (running, recent): (Vec<_>, Vec<_>) = list
                    .into_iter()
                    .partition(|j| j.status == ActivityStatus::Running);
                let has_recent = !recent.is_empty();
                view! {
                    {running.into_iter().map(|job| view! {
                        <Tab job=job active=active_id on_select=on_select />
                    }).collect_view()}
                    {has_recent.then(|| view! {
                        <div class="activity-tray-tab-divider" aria-hidden="true"></div>
                    })}
                    {recent.into_iter().take(8).map(|job| view! {
                        <Tab job=job active=active_id on_select=on_select />
                    }).collect_view()}
                }
            }}
        </div>
    }
}

#[component]
fn Tab(
    job: ActivityJobDto,
    #[prop(into)] active: Signal<Option<Uuid>>,
    on_select: StoredValue<impl Fn(Uuid) + Send + Sync + 'static>,
) -> impl IntoView {
    let id = job.stream_id;
    let status = job.status;
    let kind = kind_label(job.kind);
    let short = short_id(id);
    let dot_class = status_dot_class(status);
    let is_active = move || active.get() == Some(id);
    let title = format!("{kind} {short}");
    let dismissable = status != ActivityStatus::Running;

    view! {
        <div
            class=move || {
                if is_active() {
                    "activity-tray-tab is-active"
                } else {
                    "activity-tray-tab"
                }.to_string()
            }
            role="tab"
            aria-selected=move || if is_active() { "true" } else { "false" }
            title=title
        >
            <button
                type="button"
                class="activity-tray-tab-body"
                on:click=move |_| on_select.with_value(|f| f(id))
            >
                <span class=dot_class></span>
                <span class="activity-tray-tab-kind">{kind}</span>
                <span class="activity-tray-tab-id">{short}</span>
            </button>
            {dismissable.then(|| view! {
                <button
                    type="button"
                    class="activity-tray-tab-dismiss"
                    title="Dismiss"
                    aria-label="Dismiss job"
                    on:click=move |ev| {
                        ev.stop_propagation();
                        dismiss_job(id);
                    }
                >"×"</button>
            })}
        </div>
    }
}

#[component]
fn JobDetail(job: ActivityJobDto, #[prop(into)] url: Signal<Option<String>>) -> impl IntoView {
    let kind = kind_label(job.kind);
    let short = short_id(job.stream_id);
    let status_text = match job.status {
        ActivityStatus::Running => "running",
        ActivityStatus::Completed => "completed",
        ActivityStatus::Failed => "failed",
    };
    let started = short_time(&job.started_at);
    let finished = job
        .finished_at
        .as_deref()
        .map(short_time)
        .unwrap_or_default();
    let label = job.label.clone();
    let started_for_status = started.clone();

    view! {
        <div class="activity-tray-meta">
            <span class="activity-tray-meta-title">{label}</span>
            <span class="activity-tray-meta-sep">"·"</span>
            <span class="activity-tray-meta-kind">{kind}" "{short}</span>
            <span class="activity-tray-meta-sep">"·"</span>
            <span class="activity-tray-meta-status">{status_text}</span>
            <span class="activity-tray-meta-spacer"></span>
            <span class="activity-tray-meta-times">
                {move || if finished.is_empty() {
                    format!("started {}", started_for_status)
                } else {
                    format!("{} → {}", started, finished)
                }}
            </span>
        </div>
        <div class="activity-tray-log">
            <LogStream url=url />
        </div>
    }
}

#[component]
fn EmptyState() -> impl IntoView {
    view! {
        <div class="activity-tray-empty">
            <div class="activity-tray-empty-title">"No jobs yet"</div>
            <div class="activity-tray-empty-body">
                "Start indexing a document or generating a dataset — logs will stream here."
            </div>
        </div>
    }
}

fn kind_label(kind: ActivityKind) -> &'static str {
    match kind {
        ActivityKind::Indexing => "Indexing",
        ActivityKind::EvaluationDataset => "Dataset",
        ActivityKind::EvaluationRun => "Run",
    }
}

fn short_id(id: Uuid) -> String {
    id.to_string()[..8].to_string()
}

fn status_dot_class(status: ActivityStatus) -> &'static str {
    match status {
        ActivityStatus::Running => "activity-dot activity-dot-active",
        ActivityStatus::Completed => "activity-dot",
        ActivityStatus::Failed => "activity-dot activity-dot-fail",
    }
}

fn short_time(ts: &str) -> String {
    if ts.len() >= 19 {
        ts[11..19].to_string()
    } else if ts.len() >= 16 {
        ts[..16].replace('T', " ")
    } else {
        ts.to_string()
    }
}

fn format_hint(jobs: &[ActivityJobDto]) -> String {
    let running = jobs
        .iter()
        .filter(|j| j.status == ActivityStatus::Running)
        .count();
    let failed = jobs
        .iter()
        .filter(|j| j.status == ActivityStatus::Failed)
        .count();
    if running == 0 && failed == 0 {
        "idle".to_string()
    } else if running > 0 && failed > 0 {
        format!("{running} running · {failed} failed")
    } else if running > 0 {
        format!("{running} running")
    } else {
        format!("{failed} failed")
    }
}

#[cfg(feature = "hydrate")]
mod hydrate {
    use leptos::prelude::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    use crate::ui::components::activity::{
        clamp_tray_height, set_tray_open, use_activity_state,
    };

    static KEYBOARD_INSTALLED: AtomicBool = AtomicBool::new(false);

    pub fn install_keyboard_shortcut() {
        if KEYBOARD_INSTALLED.swap(true, Ordering::SeqCst) {
            return;
        }
        let state = use_activity_state();
        let open = state.open;
        let Some(window) = web_sys::window() else { return };

        let closure = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |ev: web_sys::KeyboardEvent| {
                if !(ev.ctrl_key() || ev.meta_key()) {
                    return;
                }
                if ev.key() != "`" {
                    return;
                }
                ev.prevent_default();
                let next = !open.get_untracked();
                set_tray_open(next);
            },
        );
        let _ = window
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    type MouseClosureCell = Rc<Cell<Option<Closure<dyn FnMut(web_sys::MouseEvent)>>>>;

    pub fn begin_resize(start_y: i32, height: RwSignal<f64>) {
        let Some(window) = web_sys::window() else { return };
        let start_height = height.get_untracked();
        let body = window.document().and_then(|d| d.body());

        if let Some(body) = body.as_ref() {
            let _ = body
                .style()
                .set_property("cursor", "row-resize");
            let _ = body
                .style()
                .set_property("user-select", "none");
        }

        let move_closure: MouseClosureCell = Rc::new(Cell::new(None));
        let up_closure: MouseClosureCell = Rc::new(Cell::new(None));

        let move_window = window.clone();
        let up_window = window.clone();
        let move_clone = move_closure.clone();
        let up_clone = up_closure.clone();
        let body_for_up = body.clone();

        let on_move = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(
            move |ev: web_sys::MouseEvent| {
                let delta = (start_y - ev.client_y()) as f64;
                let next = clamp_tray_height(start_height + delta);
                height.set(next);
            },
        );
        let _ = move_window.add_event_listener_with_callback(
            "mousemove",
            on_move.as_ref().unchecked_ref(),
        );

        let on_up = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(
            move |_ev: web_sys::MouseEvent| {
                if let Some(c) = move_clone.take() {
                    let _ = up_window.remove_event_listener_with_callback(
                        "mousemove",
                        c.as_ref().unchecked_ref(),
                    );
                    drop(c);
                }
                if let Some(c) = up_clone.take() {
                    let _ = up_window.remove_event_listener_with_callback(
                        "mouseup",
                        c.as_ref().unchecked_ref(),
                    );
                    drop(c);
                }
                if let Some(body) = body_for_up.as_ref() {
                    let _ = body.style().remove_property("cursor");
                    let _ = body.style().remove_property("user-select");
                }
            },
        );
        let _ = window
            .add_event_listener_with_callback("mouseup", on_up.as_ref().unchecked_ref());

        move_closure.set(Some(on_move));
        up_closure.set(Some(on_up));
    }
}
