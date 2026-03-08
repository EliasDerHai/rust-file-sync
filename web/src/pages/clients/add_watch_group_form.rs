use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::{ClientWatchGroupCreateDto, ServerWatchGroup};

use crate::api;
use crate::components::{Message, ToastSignal};

#[component]
pub fn AddWatchGroupForm(
    client_id: String,
    server_watch_groups: Vec<ServerWatchGroup>,
    on_created: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client_id);
    let on_created = StoredValue::new(on_created);
    let server_watch_groups = StoredValue::new(server_watch_groups);
    let show_add = RwSignal::new(false);

    let default_watch_group_id = server_watch_groups
        .get_value()
        .first()
        .map(|watch_group| watch_group.id.to_string())
        .unwrap_or_default();
    let selected_watch_group_id = RwSignal::new(default_watch_group_id);
    let path = RwSignal::new(String::new());
    let exclude_dirs_text = RwSignal::new(String::new());
    let exclude_dot = RwSignal::new(true);
    let msg = ToastSignal::new();

    let do_add = move |_| {
        let id = client_id.get_value();
        let watch_group_id_str = selected_watch_group_id.get_untracked();
        let Ok(watch_group_id) = watch_group_id_str.parse::<i64>() else {
            msg.error("Invalid watch group selection");
            return;
        };
        let path_str = path.get_untracked();
        if path_str.trim().is_empty() {
            msg.error("Path is required");
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
            server_watch_group_id: watch_group_id,
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
                    on_created.get_value()();
                }
                Err(e) => msg.error(e),
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
                        <select class="form-input" bind:value=selected_watch_group_id>
                            {move || server_watch_groups.get_value().into_iter().map(|watch_group| {
                                view! { <option value=watch_group.id.to_string()>{watch_group.name}</option> }
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
