use std::cell::RefCell;
use std::rc::Rc;
use std::str;

use js_sys::{Reflect, Uint8Array};
use leptos::wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, ReadableStreamDefaultReader, Request, RequestInit, Response};

use crate::shared::contracts::{ChatStreamDelta, ChatStreamDone, ChatStreamError, ChatStreamMeta};

#[derive(Debug, Clone)]
pub enum ChatStreamEvent {
    Meta(ChatStreamMeta),
    Delta(ChatStreamDelta),
    Done(ChatStreamDone),
    Error(ChatStreamError),
}

pub struct ChatStreamHandle {
    controller: AbortController,
}

impl ChatStreamHandle {
    pub fn cancel(&self) {
        self.controller.abort();
    }
}

pub struct ChatStreamCallbacks {
    pub on_event: Box<dyn Fn(ChatStreamEvent)>,
    pub on_done: Box<dyn FnOnce()>,
    pub on_fail: Box<dyn FnOnce(String)>,
}

pub fn start_chat_stream(
    url: &str,
    body_json: &str,
    callbacks: ChatStreamCallbacks,
) -> Result<ChatStreamHandle, String> {
    let controller = AbortController::new().map_err(|e| format!("abort controller: {e:?}"))?;
    let signal = controller.signal();

    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_signal(Some(&signal));
    opts.set_body(&JsValue::from_str(body_json));

    let headers = web_sys::Headers::new().map_err(|e| format!("headers new: {e:?}"))?;
    headers
        .set("content-type", "application/json")
        .map_err(|e| format!("set content-type: {e:?}"))?;
    headers
        .set("accept", "text/event-stream")
        .map_err(|e| format!("set accept: {e:?}"))?;
    opts.set_headers(&headers);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| format!("request create failed: {e:?}"))?;

    let window = web_sys::window().ok_or("no window")?;
    let fetch_promise = window.fetch_with_request(&request);

    let callbacks = Rc::new(RefCell::new(Some(callbacks)));

    leptos::task::spawn_local({
        let callbacks = callbacks.clone();
        async move {
            let Ok(value) = JsFuture::from(fetch_promise).await else {
                invoke_fail(&callbacks, "fetch aborted or failed".to_string());
                return;
            };

            let Ok(response) = value.dyn_into::<Response>() else {
                invoke_fail(&callbacks, "response cast failed".to_string());
                return;
            };
            if !response.ok() {
                invoke_fail(&callbacks, format!("request failed: {}", response.status()));
                return;
            }
            let Some(body) = response.body() else {
                invoke_fail(&callbacks, "response had no body".to_string());
                return;
            };
            let Ok(reader) = body.get_reader().dyn_into::<ReadableStreamDefaultReader>() else {
                invoke_fail(&callbacks, "reader cast failed".to_string());
                return;
            };

            consume_reader(reader, callbacks).await;
        }
    });

    Ok(ChatStreamHandle { controller })
}

async fn consume_reader(
    reader: ReadableStreamDefaultReader,
    callbacks: Rc<RefCell<Option<ChatStreamCallbacks>>>,
) {
    let mut buffer = String::new();

    loop {
        let promise = reader.read();
        let Ok(result) = JsFuture::from(promise).await else {
            invoke_fail(&callbacks, "stream read failed".to_string());
            return;
        };
        let done = Reflect::get(&result, &JsValue::from_str("done"))
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false);
        if done {
            break;
        }
        let Ok(value) = Reflect::get(&result, &JsValue::from_str("value")) else {
            continue;
        };
        if value.is_undefined() || value.is_null() {
            continue;
        }
        let array = Uint8Array::new(&value);
        let bytes = array.to_vec();
        let Ok(text) = str::from_utf8(&bytes) else {
            invoke_fail(&callbacks, "stream chunk not utf-8".to_string());
            return;
        };
        normalise_into(text, &mut buffer);
        drain_frames(&mut buffer, &callbacks);
    }

    invoke_done(&callbacks);
}

fn drain_frames(buffer: &mut String, callbacks: &Rc<RefCell<Option<ChatStreamCallbacks>>>) {
    while let Some(idx) = buffer.find("\n\n") {
        let mut block: String = buffer.drain(..idx + 2).collect();
        block.truncate(block.len() - 2);

        let mut event_name = String::from("message");
        let mut data = String::new();
        for line in block.split('\n') {
            if let Some(rest) = line.strip_prefix("event:") {
                event_name = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("data:") {
                let payload = rest.strip_prefix(' ').unwrap_or(rest);
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(payload);
            }
        }
        if data.is_empty() {
            continue;
        }
        if let Some(event) = decode_event(&event_name, &data) {
            invoke_event(callbacks, event);
        }
    }
}

fn decode_event(name: &str, data: &str) -> Option<ChatStreamEvent> {
    match name {
        "meta" => serde_json::from_str::<ChatStreamMeta>(data)
            .ok()
            .map(ChatStreamEvent::Meta),
        "delta" => serde_json::from_str::<ChatStreamDelta>(data)
            .ok()
            .map(ChatStreamEvent::Delta),
        "done" => serde_json::from_str::<ChatStreamDone>(data)
            .ok()
            .map(ChatStreamEvent::Done),
        "error" => serde_json::from_str::<ChatStreamError>(data)
            .ok()
            .map(ChatStreamEvent::Error),
        _ => None,
    }
}

fn normalise_into(src: &str, dest: &mut String) {
    for ch in src.chars() {
        if ch == '\r' {
            dest.push('\n');
        } else {
            dest.push(ch);
        }
    }
}

fn invoke_event(callbacks: &Rc<RefCell<Option<ChatStreamCallbacks>>>, event: ChatStreamEvent) {
    if let Some(cb) = callbacks.borrow().as_ref() {
        (cb.on_event)(event);
    }
}

fn invoke_done(callbacks: &Rc<RefCell<Option<ChatStreamCallbacks>>>) {
    if let Some(cb) = callbacks.borrow_mut().take() {
        (cb.on_done)();
    }
}

fn invoke_fail(callbacks: &Rc<RefCell<Option<ChatStreamCallbacks>>>, message: String) {
    if let Some(cb) = callbacks.borrow_mut().take() {
        (cb.on_fail)(message);
    }
}
