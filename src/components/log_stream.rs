use leptos::prelude::*;

use crate::shared::{LogEvent, LogLevel};

#[component]
pub fn LogStream(#[prop(into)] url: Signal<Option<String>>) -> impl IntoView {
    let (events, set_events) = signal::<Vec<LogEvent>>(Vec::new());
    let (running, set_running) = signal(false);

    #[cfg(feature = "hydrate")]
    {
        let handle = StoredValue::new(None::<self::hydrate::StreamHandle>);
        Effect::new(move |_| {
            if let Some(url) = url.get() {
                set_events.set(Vec::new());
                set_running.set(true);
                handle.set_value(Some(self::hydrate::open(url, set_events, set_running)));
            } else {
                handle.set_value(None);
                set_events.set(Vec::new());
                set_running.set(false);
            }
        });
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = url;
        let _ = (set_events, set_running);
    }

    view! {
        <div class="log-stream">
            {move || (running.get() && events.with(|e| e.is_empty())).then(|| view! {
                <div class="faint text-xs italic px-3 py-2">"Streaming…"</div>
            })}
            {move || {
                let evs = events.get();
                if evs.is_empty() && !running.get() {
                    return view! {
                        <div class="faint text-xs italic px-3 py-2">"No log output."</div>
                    }.into_any();
                }
                evs.into_iter().map(|e| view! { <LogLine event=e /> }).collect_view().into_any()
            }}
        </div>
    }
}

#[component]
fn LogLine(event: LogEvent) -> impl IntoView {
    let (prefix, cls) = match event.level {
        LogLevel::Info => ("INFO", "log-line-info"),
        LogLevel::Warn => ("WARN", "log-line-warn"),
        LogLevel::Error => ("ERRO", "log-line-error"),
        LogLevel::Success => ("DONE", "log-line-success"),
    };
    view! {
        <div class="log-stream-line">
            <span class=format!("log-stream-level {cls}")>{prefix}</span>
            <span class=cls>{event.message}</span>
        </div>
    }
}

#[cfg(feature = "hydrate")]
mod hydrate {
    use leptos::prelude::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{EventSource, MessageEvent};

    use crate::shared::{LogEvent, LogLevel};

    pub struct StreamHandle {
        source: EventSource,
    }

    impl Drop for StreamHandle {
        fn drop(&mut self) {
            self.source.close();
        }
    }

    pub fn open(
        url: String,
        set_events: WriteSignal<Vec<LogEvent>>,
        set_running: WriteSignal<bool>,
    ) -> StreamHandle {
        let source = match EventSource::new(&url) {
            Ok(s) => s,
            Err(err) => {
                set_events.update(|evs| {
                    evs.push(LogEvent {
                        level: LogLevel::Error,
                        message: format!("failed to open stream: {err:?}"),
                    })
                });
                set_running.set(false);

                return StreamHandle {
                    source: EventSource::new("about:blank")
                        .unwrap_or_else(|_| EventSource::new(&url).expect("event source")),
                };
            }
        };

        let source_for_msg = source.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |evt: MessageEvent| {
            let data = evt.data().as_string().unwrap_or_default();
            if data == "__done__" {
                set_running.set(false);
                source_for_msg.close();
                return;
            }
            match serde_json::from_str::<LogEvent>(&data) {
                Ok(e) => set_events.update(|evs| evs.push(e)),
                Err(err) => set_events.update(|evs| {
                    evs.push(LogEvent {
                        level: LogLevel::Error,
                        message: format!("malformed log event: {err}"),
                    })
                }),
            }
        });
        source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        let source_for_err = source.clone();
        let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            set_running.set(false);
            source_for_err.close();
        });
        source.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        StreamHandle { source }
    }
}
