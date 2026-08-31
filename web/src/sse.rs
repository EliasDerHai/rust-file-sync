use leptos::{logging::{log, warn}, prelude::*};
use shared::dtos::ServerEventDto;
use shared::endpoint::ServerEndpoint;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, MessageEvent};

/// Live state of the app-wide SSE connection to the server, shown as a status dot
/// in the navbar. Distinct from `/client`'s disk-poll sync - this is server -> web push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Failed,
}

#[derive(Clone, Copy)]
pub struct ConnectionStatusSignal(pub RwSignal<ConnectionStatus>);

/// Opens the SSE connection for the lifetime of the app and returns a signal tracking
/// its status. Intended to be called once from the root component and provided as context.
pub fn init() -> ConnectionStatusSignal {
    let status = RwSignal::new(ConnectionStatus::Connecting);
    let source = EventSource::new(ServerEndpoint::ApiEvents.to_str())
        .expect("failed to open SSE connection");
    let onopen = Closure::<dyn FnMut()>::new(move || {
        status.set(ConnectionStatus::Connected);
    });

    source.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let onerror = Closure::<dyn FnMut()>::new(move || {
        status.set(ConnectionStatus::Failed);
    });
    source.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let Some(text) = event.data().as_string() else {
            return;
        };
        match serde_json::from_str::<ServerEventDto>(&text) {
            Ok(ServerEventDto::ServerHello) => {
                log!("SSE: received server-hello");
            }
            Err(err) => {
                warn!("SSE: failed to parse event {text:?}: {err}");
            }
        }
    });
    source.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // The EventSource is meant to live for the whole app session; leak it rather than
    // tying it to a value someone could accidentally drop.
    std::mem::forget(source);

    ConnectionStatusSignal(status)
}
