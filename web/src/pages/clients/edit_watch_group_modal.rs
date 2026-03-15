use leptos::prelude::*;
use shared::dtos::ClientWatchGroupUpdateDto;

use crate::api;
use crate::components::Modal;

#[component]
pub fn EditWatchGroupModal(
    show: RwSignal<bool>,
    client_id: String,
    watch_group_id: i64,
    initial_path: String,
    initial_exclude_dirs: String,
    initial_exclude_dot: bool,
    on_saved: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id_sv = StoredValue::new(client_id);
    let path = RwSignal::new(initial_path);
    let exclude_dirs_text = RwSignal::new(initial_exclude_dirs);
    let exclude_dot = RwSignal::new(initial_exclude_dot);

    let on_save = move || {
        let id = client_id_sv.get_value();
        let path_str = path.get_untracked();
        let excl_text = exclude_dirs_text.get_untracked();
        let excl_dot = exclude_dot.get_untracked();
        async move {
            if path_str.trim().is_empty() {
                return Err("Path is required".to_string());
            }
            let excl: Vec<String> = excl_text
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let dto = ClientWatchGroupUpdateDto {
                path_to_monitor: path_str,
                exclude_dirs: excl,
                exclude_dot_dirs: excl_dot,
            };
            api::update_client_watch_group(&id, watch_group_id, &dto).await
        }
    };

    view! {
        <Modal show title="Edit Watch Group Assignment" on_save on_saved>
            <div class="form-group">
                <label>"Path"</label>
                <input type="text" class="form-input" bind:value=path />
            </div>
            <div class="form-group">
                <label>"Exclude dirs (one per line)"</label>
                <textarea class="form-input" rows="3" bind:value=exclude_dirs_text />
            </div>
            <div class="checkbox-group" style="margin-bottom: 1.25rem;">
                <input type="checkbox" bind:checked=exclude_dot />
                <label>"Exclude dot dirs"</label>
            </div>
        </Modal>
    }
}
