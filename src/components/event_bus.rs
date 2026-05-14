use leptos::prelude::*;

use crate::shared::PublishedEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Open,
    Closed,
}

#[derive(Clone, Copy)]
pub struct EventBus {
    pub last_event: ReadSignal<Option<PublishedEvent>>,
    pub epoch: ReadSignal<u32>,
    pub connection: ReadSignal<ConnectionState>,
}

pub fn provide_event_bus() {
    let (last_event, set_last_event) = signal::<Option<PublishedEvent>>(None);
    let (epoch, set_epoch) = signal(0u32);
    let (connection, set_connection) = signal(ConnectionState::Connecting);

    let bus = EventBus {
        last_event,
        epoch,
        connection,
    };

    #[cfg(feature = "hydrate")]
    {
        Effect::new(move |prev: Option<()>| {
            if prev.is_none() {
                self::hydrate::connect(set_last_event, set_epoch, set_connection);
            }
        });
    }

    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (set_last_event, set_epoch, set_connection);
    }

    provide_context(bus);
}

pub fn use_event_bus() -> EventBus {
    use_context::<EventBus>().expect("EventBus context must be provided in App")
}

pub fn use_invalidator<F>(predicate: F) -> Memo<u32>
where
    F: Fn(&PublishedEvent) -> bool + Send + Sync + 'static,
{
    let bus = use_event_bus();
    let (count, set_count) = signal(0u32);

    Effect::new(move |prev_epoch: Option<u32>| {
        let epoch = bus.epoch.get();
        let event = bus.last_event.get();

        if let Some(prev) = prev_epoch {
            if prev != epoch {
                set_count.update(|c| *c = c.wrapping_add(1));
            }
        }

        if let Some(ref e) = event {
            if predicate(e) {
                set_count.update(|c| *c = c.wrapping_add(1));
            }
        }

        epoch
    });

    Memo::new(move |_| count.get())
}

#[cfg(feature = "hydrate")]
mod hydrate {
    use std::cell::RefCell;

    use leptos::prelude::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{CloseEvent, MessageEvent, WebSocket};

    use super::{ConnectionState, PublishedEvent};

    thread_local! {
        static ACTIVE_SOCKET: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
    }

    pub fn connect(
        set_last_event: WriteSignal<Option<PublishedEvent>>,
        set_epoch: WriteSignal<u32>,
        set_connection: WriteSignal<ConnectionState>,
    ) {
        open_socket(set_last_event, set_epoch, set_connection, 0);
    }

    fn open_socket(
        set_last_event: WriteSignal<Option<PublishedEvent>>,
        set_epoch: WriteSignal<u32>,
        set_connection: WriteSignal<ConnectionState>,
        attempt: u32,
    ) {
        let url = match build_url() {
            Some(u) => u,
            None => {
                set_connection.set(ConnectionState::Closed);
                return;
            }
        };

        set_connection.set(ConnectionState::Connecting);

        let ws = match WebSocket::new(&url) {
            Ok(ws) => ws,
            Err(e) => {
                tracing_error(&format!("event bus: failed to open websocket: {e:?}"));
                set_connection.set(ConnectionState::Closed);
                schedule_reconnect(set_last_event, set_epoch, set_connection, attempt + 1);
                return;
            }
        };

        let on_open = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            set_connection.set(ConnectionState::Open);
            set_epoch.update(|e| *e = e.wrapping_add(1));
        });
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |evt: MessageEvent| {
            let data = match evt.data().as_string() {
                Some(s) => s,
                None => return,
            };
            match serde_json::from_str::<PublishedEvent>(&data) {
                Ok(event) => set_last_event.set(Some(event)),
                Err(err) => tracing_error(&format!("event bus: malformed payload: {err}")),
            }
        });
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        let reconnect = move || {
            ACTIVE_SOCKET.with(|socket| {
                socket.borrow_mut().take();
            });
            set_connection.set(ConnectionState::Closed);
            schedule_reconnect(set_last_event, set_epoch, set_connection, attempt + 1);
        };

        let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |_evt: CloseEvent| {
            reconnect();
        });
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {});
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        ACTIVE_SOCKET.with(|socket| {
            *socket.borrow_mut() = Some(ws);
        });
    }

    fn schedule_reconnect(
        set_last_event: WriteSignal<Option<PublishedEvent>>,
        set_epoch: WriteSignal<u32>,
        set_connection: WriteSignal<ConnectionState>,
        attempt: u32,
    ) {
        let delay_ms = ((1u32 << attempt.min(6)) * 500).min(30_000) as i32;

        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        let cb = Closure::<dyn FnMut()>::new(move || {
            open_socket(set_last_event, set_epoch, set_connection, attempt);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            delay_ms,
        );
        cb.forget();
    }

    fn build_url() -> Option<String> {
        let window = web_sys::window()?;
        let location = window.location();
        let host = location.host().ok()?;
        let protocol = location.protocol().ok()?;
        let scheme = if protocol == "https:" { "wss" } else { "ws" };
        Some(format!("{scheme}://{host}/api/events/ws"))
    }

    fn tracing_error(msg: &str) {
        web_sys::console::error_1(&msg.into());
    }
}
