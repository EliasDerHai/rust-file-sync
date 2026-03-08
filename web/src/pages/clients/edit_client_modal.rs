use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::ClientUpdateDto;

use crate::api;
use crate::components::Message;

#[component]
pub fn EditClientModal(
    show: RwSignal<bool>,
    client_id: String,
    current_poll_ms: u16,
    on_saved: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client_id);
    let on_saved_sv = StoredValue::new(on_saved);

    let poll_value = RwSignal::new(current_poll_ms.to_string());
    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    let do_save = move |_| {
        let id = client_id.get_value();
        let ms_str = poll_value.get_untracked();
        let Ok(ms) = ms_str.parse::<u16>() else {
            set_msg.set(Some((false, "Invalid poll interval".to_string())));
            return;
        };
        let dto = ClientUpdateDto {
            min_poll_interval_in_ms: ms,
        };
        spawn_local(async move {
            match api::update_client(&id, &dto).await {
                Ok(()) => {
                    show.set(false);
                    on_saved_sv.get_value()();
                }
                Err(e) => set_msg.set(Some((false, format!("Error: {e}")))),
            }
        });
    };

    view! {
        <Show when=move || show.get()>
            <div class="dialog-overlay" on:click=move |_| show.set(false)>
                <div class="dialog" on:click=|e| e.stop_propagation()>
                    <h2 class="dialog-title">"Edit Client Poll Interval"</h2>
                    <div class="form-group">
                        <label>"Poll interval (ms)"</label>
                        <input
                            type="number"
                            class="form-input"
                            style="width: 160px;"
                            bind:value=poll_value
                        />
                    </div>
                    <Message signal=msg />
                    <div class="dialog-actions">
                        <button class="btn btn-secondary" on:click=move |_| show.set(false)>
                            "Cancel"
                        </button>
                        <button class="btn btn-success" on:click=do_save>
                            "Save"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
