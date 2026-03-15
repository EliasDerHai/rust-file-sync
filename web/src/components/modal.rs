use std::future::Future;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::{Message, ToastSignal};

#[component]
pub fn Modal<FnSave, FnSaved, Fut>(
    show: RwSignal<bool>,
    on_save: FnSave,
    on_saved: FnSaved,
    children: ChildrenFn,
    #[prop(optional, into)] title: Option<String>,
    #[prop(optional, into)] confirm_label: Option<String>,
    #[prop(optional, into)] cancel_label: Option<String>,
) -> impl IntoView
where
    FnSave: Fn() -> Fut + 'static + Send + Sync,
    FnSaved: Fn() + 'static + Clone + Send + Sync,
    Fut: Future<Output = Result<(), String>> + 'static,
{
    let msg = ToastSignal::new();
    let on_save_sv = StoredValue::new(on_save);
    let on_saved_sv = StoredValue::new(on_saved);

    let do_confirm = move |_| {
        let future = on_save_sv.with_value(|f| f());
        spawn_local(async move {
            match future.await {
                Ok(()) => {
                    show.set(false);
                    on_saved_sv.get_value()();
                }
                Err(e) => msg.error(e),
            }
        });
    };

    let confirm_label = confirm_label.unwrap_or("Save".to_string());
    let cancel_label = cancel_label.unwrap_or("Cancel".to_string());

    view! {
        <Show when=move || show.get()>
            <div class="dialog-overlay" on:click=move |_| show.set(false)>
                <div class="dialog" on:click=|e| e.stop_propagation()>
                    <Message signal=msg />
                    {
                        if let Some(ref t) = title {
                            view! {<h2 class="dialog-title">{t.clone()}</h2>}.into_any()
                        } else {
                            view! {<span />}.into_any()
                        }
                    }
                    {children()}
                    <div class="dialog-actions">
                        <button class="btn btn-secondary" on:click=move |_| show.set(false)>
                           {cancel_label.clone()}
                        </button>
                        <button class="btn btn-success" on:click=do_confirm>
                           {confirm_label.clone()}
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
