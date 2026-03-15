use leptos::prelude::*;
use shared::dtos::ClientUpdateDto;

use crate::api;
use crate::components::Modal;

#[component]
pub fn EditClientModal(
    show: RwSignal<bool>,
    client_id: String,
    current_poll_ms: u16,
    on_saved: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id_sv = StoredValue::new(client_id);
    let poll_value = RwSignal::new(current_poll_ms.to_string());

    let on_save = move || {
        let id = client_id_sv.get_value();
        let ms_str = poll_value.get_untracked();
        async move {
            let Ok(ms) = ms_str.parse::<u16>() else {
                return Err("Invalid poll interval".to_string());
            };
            let dto = ClientUpdateDto {
                min_poll_interval_in_ms: ms,
            };
            api::update_client(&id, &dto).await
        }
    };

    view! {
        <Modal show title="Edit Client Poll Interval" on_save on_saved>
            <div class="form-group">
                <label>"Poll interval (ms)"</label>
                <input
                    type="number"
                    class="form-input"
                    style="width: 160px;"
                    bind:value=poll_value
                />
            </div>
        </Modal>
    }
}
