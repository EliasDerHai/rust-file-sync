use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::{ClientWatchGroupCreateDto, ServerWatchGroup};

use crate::api;
use crate::components::Message;

#[component]
pub fn AddWatchGroupForm(
    client_id: String,
    server_wgs: Vec<ServerWatchGroup>,
    on_created: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client_id);
    let on_created_sv = StoredValue::new(on_created);
    let server_wgs_sv = StoredValue::new(server_wgs);
    let show_add = RwSignal::new(false);

    let default_wg_id = server_wgs_sv
        .get_value()
        .first()
        .map(|wg| wg.id.to_string())
        .unwrap_or_default();
    let selected_wg_id = RwSignal::new(default_wg_id);
    let path = RwSignal::new(String::new());
    let exclude_dirs_text = RwSignal::new(String::new());
    let exclude_dot = RwSignal::new(true);
    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    let do_add = move |_| {
        let id = client_id.get_value();
        let wg_id_str = selected_wg_id.get_untracked();
        let Ok(wg_id) = wg_id_str.parse::<i64>() else {
            set_msg.set(Some((false, "Invalid watch group selection".to_string())));
            return;
        };
        let path_str = path.get_untracked();
        if path_str.trim().is_empty() {
            set_msg.set(Some((false, "Path is required".to_string())));
            return;
        }
        let excl: Vec<String> = exclude_dirs_text
            .get_untracked()
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let excl_dot = exclude_dot.get_untracked();
        let dto = ClientWatchGroupCreateDto {
            server_watch_group_id: wg_id,
            path_to_monitor: path_str,
            exclude_dirs: excl,
            exclude_dot_dirs: excl_dot,
        };
        spawn_local(async move {
            match api::create_client_watch_group(&id, &dto).await {
                Ok(()) => {
                    path.set(String::new());
                    exclude_dirs_text.set(String::new());
                    show_add.set(false);
                    on_created_sv.get_value()();
                }
                Err(e) => set_msg.set(Some((false, format!("Error: {e}")))),
            }
        });
    };

    view! {
        <div style="margin-top: 0.75rem;">
            <button
                class="btn btn-secondary"
                on:click=move |_| show_add.update(|v| *v = !*v)
            >
                "+ Add Watch Group"
            </button>
            <Show when=move || show_add.get()>
                <div style="margin-top: 0.75rem;">
                    <div class="form-group">
                        <label>"Watch Group"</label>
                        <select class="form-input" bind:value=selected_wg_id>
                            {move || server_wgs_sv.get_value().into_iter().map(|wg| {
                                view! { <option value=wg.id.to_string()>{wg.name}</option> }
                            }).collect_view()}
                        </select>
                    </div>
                    <div class="form-group">
                        <label>"Path"</label>
                        <input type="text" class="form-input" bind:value=path />
                    </div>
                    <div class="form-group">
                        <label>"Exclude dirs (one per line)"</label>
                        <textarea class="form-input" rows="3" bind:value=exclude_dirs_text />
                    </div>
                    <div class="checkbox-group">
                        <input type="checkbox" bind:checked=exclude_dot />
                        <label>"Exclude dot dirs"</label>
                    </div>
                    <div class="flex gap-1" style="margin-top: 0.75rem;">
                        <button class="btn btn-success" on:click=do_add>"Add"</button>
                        <button
                            class="btn btn-secondary"
                            on:click=move |_| show_add.set(false)
                        >
                            "Cancel"
                        </button>
                    </div>
                    <Message signal=msg />
                </div>
            </Show>
        </div>
    }
}
