use gloo_timers::callback::Timeout;
use leptos::prelude::*;

#[derive(Debug, Clone)]
enum ToastKind {
    Success,
    Error,
}

#[derive(Debug, Clone, Default)]
struct Toast(Option<InnerToast>);

#[derive(Debug, Clone)]
struct InnerToast {
    message: String,
    toast_kind: ToastKind,
}

#[derive(Clone, Copy)]
pub struct ToastSignal(RwSignal<Toast>);

impl ToastSignal {
    pub fn new() -> Self {
        Self(RwSignal::new(Toast::default()))
    }

    pub fn error(self, message: impl Into<String>) {
        self.0.set(Toast(Some(InnerToast {
            message: message.into(),
            toast_kind: ToastKind::Error,
        })));
    }

    pub fn success(self, message: impl Into<String>) {
        self.0.set(Toast(Some(InnerToast {
            message: message.into(),
            toast_kind: ToastKind::Success,
        })));
        Timeout::new(3_000, move || self.clear()).forget();
    }

    pub fn clear(self) {
        self.0.set(Toast::default());
    }
}

#[component]
pub fn Message(signal: ToastSignal) -> impl IntoView {
    let inner = signal.0;
    view! {
        <Show when=move || inner.get().0.is_some()>
            {move || {
                inner.get().0.map(|InnerToast { message, toast_kind }| {
                    let class = match toast_kind {
                        ToastKind::Success => "message message-success",
                        ToastKind::Error => "message message-error",
                    };
                    view! { <div class=class>{message}</div> }
                })
            }}
        </Show>
    }
}
