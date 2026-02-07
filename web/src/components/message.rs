use leptos::prelude::*;

/// Displays a success or error message. Signal contains Option<(is_success, text)>.
#[component]
pub fn Message(
    signal: ReadSignal<Option<(bool, String)>>,
) -> impl IntoView {
    view! {
        <Show when=move || signal.get().is_some()>
            {move || {
                signal.get().map(|(is_success, text)| {
                    let class = if is_success {
                        "message message-success"
                    } else {
                        "message message-error"
                    };
                    view! { <div class=class>{text}</div> }
                })
            }}
        </Show>
    }
}
