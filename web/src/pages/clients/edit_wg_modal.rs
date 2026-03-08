use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::ClientWatchGroupUpdateDto;

use crate::api;
use crate::components::Message;

#[component]
pub fn EditWatchGroupModal(
    show: RwSignal<bool>,
    client_id: String,
    wg_id: i64,
    initial_path: String,
    initial_exclude_dirs: String,
    initial_exclude_dot: bool,
    on_saved: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client_id);
    let on_saved_sv = StoredValue::new(on_saved);

    let path = RwSignal::new(initial_path);
    let exclude_dirs_text = RwSignal::new(initial_exclude_dirs);
    let exclude_dot = RwSignal::new(initial_exclude_dot);
    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    let do_save = move |_| {
        let id = client_id.get_value();
        let path_str = path.get_untracked();
        let excl: Vec<String> = exclude_dirs_text
            .get_untracked()
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let excl_dot = exclude_dot.get_untracked();
        let dto = ClientWatchGroupUpdateDto {
            path_to_monitor: path_str,
            exclude_dirs: excl,
            exclude_dot_dirs: excl_dot,
        };
        spawn_local(async move {
            match api::update_client_watch_group(&id, wg_id, &dto).await {
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
                <div class="dialog" on:click=|e| e.stop_propagation() style="min-width: 420px;">
                    <h2 class="dialog-title">"Edit Watch Group Assignment"</h2>
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
