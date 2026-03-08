use leptos::prelude::*;

#[component]
pub fn ConfirmDialog(
    show: RwSignal<bool>,
    message: String,
    on_confirm: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let on_confirm = StoredValue::new(on_confirm);

    view! {
        <Show when=move || show.get()>
            <div class="dialog-overlay" on:click=move |_| show.set(false)>
                <div class="dialog" on:click=|e| e.stop_propagation()>
                    <p>{message.clone()}</p>
                    <div class="dialog-actions">
                        <button class="btn btn-secondary" on:click=move |_| show.set(false)>
                            "Cancel"
                        </button>
                        <button
                            class="btn btn-danger"
                            on:click=move |_| {
                                on_confirm.get_value()();
                                show.set(false);
                            }
                        >
                            "Delete"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
