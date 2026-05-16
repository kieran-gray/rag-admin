use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

use crate::contracts::{
    classify_event, ActivityDelta, ActivityJobDto, ActivityStart, ActivityStatus,
};
use crate::server_functions::jobs::list_active_jobs;
use crate::ui::components::event_bus::use_event_bus;

const DEFAULT_TRAY_HEIGHT_PX: f64 = 320.0;
const MIN_TRAY_HEIGHT_PX: f64 = 140.0;
const MAX_TRAY_HEIGHT_FRACTION: f64 = 0.75;

#[cfg(feature = "hydrate")]
const LS_OPEN_KEY: &str = "rag-admin.tray.open";
#[cfg(feature = "hydrate")]
const LS_HEIGHT_KEY: &str = "rag-admin.tray.height";

#[derive(Clone, Copy)]
pub struct ActivityState {
    pub rows: RwSignal<Vec<ActivityJobDto>>,
    pub open: RwSignal<bool>,
    pub selected: RwSignal<Option<Uuid>>,
    pub height: RwSignal<f64>,
}

impl ActivityState {
    pub fn running_count(&self) -> usize {
        self.rows.with(|rows| {
            rows.iter()
                .filter(|r| r.status == ActivityStatus::Running)
                .count()
        })
    }

    pub fn running_jobs(&self) -> Vec<ActivityJobDto> {
        self.rows.with(|rows| {
            let mut list: Vec<_> = rows
                .iter()
                .filter(|r| r.status == ActivityStatus::Running)
                .cloned()
                .collect();
            list.sort_by(|a, b| a.started_at.cmp(&b.started_at));
            list
        })
    }

    pub fn ordered_jobs(&self) -> Vec<ActivityJobDto> {
        self.rows.with(|rows| {
            let mut list = rows.clone();
            list.sort_by(|a, b| match (a.status, b.status) {
                (ActivityStatus::Running, ActivityStatus::Running) => {
                    a.started_at.cmp(&b.started_at)
                }
                (ActivityStatus::Running, _) => std::cmp::Ordering::Less,
                (_, ActivityStatus::Running) => std::cmp::Ordering::Greater,
                _ => b
                    .finished_at
                    .cmp(&a.finished_at)
                    .then_with(|| b.started_at.cmp(&a.started_at)),
            });
            list
        })
    }
}

pub fn provide_activity_state() {
    let rows = RwSignal::new(Vec::<ActivityJobDto>::new());
    let initial_open = read_stored_open();
    let initial_height = read_stored_height();
    let open = RwSignal::new(initial_open);
    let selected = RwSignal::new(None::<Uuid>);
    let height = RwSignal::new(initial_height);

    let state = ActivityState {
        rows,
        open,
        selected,
        height,
    };
    provide_context(state);
    let set_rows = rows.write_only();

    persist_open_on_change(open);
    persist_height_on_change(height);

    Effect::new(move |prev: Option<()>| {
        if prev.is_none() {
            spawn_local(async move {
                match list_active_jobs().await {
                    Ok(snapshot) => set_rows.set(snapshot),
                    Err(err) => {
                        leptos::logging::warn!("list_active_jobs failed: {err}");
                    }
                }
            });
        }
    });

    let bus = use_event_bus();
    Effect::new(move |_| {
        if let Some(event) = bus.last_event.get() {
            if let Some(delta) = classify_event(&event) {
                let needs_url_refresh = matches!(
                    &delta,
                    ActivityDelta::Start(_) | ActivityDelta::Refresh { .. }
                );
                set_rows.update(|rows| apply_delta(rows, delta));
                if needs_url_refresh {
                    spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(150).await;
                        if let Ok(snapshot) = list_active_jobs().await {
                            set_rows.set(snapshot);
                        }
                    });
                }
            }
        }
    });

    Effect::new(move |prev_epoch: Option<u32>| {
        let epoch = bus.epoch.get();
        if prev_epoch.is_some() && prev_epoch != Some(epoch) {
            spawn_local(async move {
                if let Ok(snapshot) = list_active_jobs().await {
                    set_rows.set(snapshot);
                }
            });
        }
        epoch
    });
}

pub fn use_activity_state() -> ActivityState {
    use_context::<ActivityState>().expect("ActivityState context must be provided in App")
}

pub fn set_tray_open(open: bool) {
    let state = use_activity_state();
    state.open.set(open);
    if open && state.selected.get_untracked().is_none() {
        if let Some(first) = state.running_jobs().into_iter().next() {
            state.selected.set(Some(first.stream_id));
        } else if let Some(first) = state.ordered_jobs().into_iter().next() {
            state.selected.set(Some(first.stream_id));
        }
    }
}

pub fn open_tray_with(stream_id: Uuid) {
    let state = use_activity_state();
    state.selected.set(Some(stream_id));
    state.open.set(true);
}

pub fn dismiss_job(stream_id: Uuid) {
    let state = use_activity_state();
    state
        .rows
        .update(|rows| rows.retain(|r| r.stream_id != stream_id));
    if state.selected.get_untracked() == Some(stream_id) {
        state.selected.set(None);
    }
}

pub fn clamp_tray_height(height: f64) -> f64 {
    let max = max_tray_height();
    height.clamp(MIN_TRAY_HEIGHT_PX, max)
}

fn max_tray_height() -> f64 {
    #[cfg(feature = "hydrate")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(value) = window.inner_height() {
                if let Some(h) = value.as_f64() {
                    return (h * MAX_TRAY_HEIGHT_FRACTION).max(MIN_TRAY_HEIGHT_PX);
                }
            }
        }
    }
    900.0 * MAX_TRAY_HEIGHT_FRACTION
}

fn apply_delta(rows: &mut Vec<ActivityJobDto>, delta: ActivityDelta) {
    match delta {
        ActivityDelta::Start(ActivityStart {
            stream_id,
            aggregate_type,
            kind,
            label,
            started_at,
        }) => match rows.iter_mut().find(|r| r.stream_id == stream_id) {
            Some(row) => {
                row.status = ActivityStatus::Running;
                row.started_at = started_at;
                row.finished_at = None;
                row.label = label;
                row.aggregate_type = aggregate_type;
                row.kind = kind;
            }
            None => rows.push(ActivityJobDto {
                stream_id,
                aggregate_type,
                kind,
                label,
                status: ActivityStatus::Running,
                started_at,
                finished_at: None,
                stream_url: None,
            }),
        },
        ActivityDelta::Complete {
            stream_id,
            occurred_at,
        } => {
            if let Some(row) = rows.iter_mut().find(|r| r.stream_id == stream_id) {
                row.status = ActivityStatus::Completed;
                row.finished_at = Some(occurred_at);
            }
        }
        ActivityDelta::Fail {
            stream_id,
            occurred_at,
        } => {
            if let Some(row) = rows.iter_mut().find(|r| r.stream_id == stream_id) {
                row.status = ActivityStatus::Failed;
                row.finished_at = Some(occurred_at);
            }
        }
        ActivityDelta::Remove { stream_id } => {
            rows.retain(|r| r.stream_id != stream_id);
        }
        ActivityDelta::Refresh { .. } => {}
    }
}

#[cfg(feature = "hydrate")]
fn read_stored_open() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|store| store.get_item(LS_OPEN_KEY).ok().flatten())
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[cfg(not(feature = "hydrate"))]
fn read_stored_open() -> bool {
    false
}

#[cfg(feature = "hydrate")]
fn read_stored_height() -> f64 {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|store| store.get_item(LS_HEIGHT_KEY).ok().flatten())
        .and_then(|v| v.parse::<f64>().ok())
        .map(clamp_tray_height)
        .unwrap_or(DEFAULT_TRAY_HEIGHT_PX)
}

#[cfg(not(feature = "hydrate"))]
fn read_stored_height() -> f64 {
    DEFAULT_TRAY_HEIGHT_PX
}

#[cfg(feature = "hydrate")]
fn persist_open_on_change(open: RwSignal<bool>) {
    Effect::new(move |_| {
        let value = open.get();
        if let Some(store) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = store.set_item(LS_OPEN_KEY, if value { "1" } else { "0" });
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn persist_open_on_change(_open: RwSignal<bool>) {}

#[cfg(feature = "hydrate")]
fn persist_height_on_change(height: RwSignal<f64>) {
    Effect::new(move |_| {
        let value = height.get();
        if let Some(store) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = store.set_item(LS_HEIGHT_KEY, &format!("{value:.0}"));
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn persist_height_on_change(_height: RwSignal<f64>) {}
