use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::ClientWatchGroupDto;

use crate::api;
use crate::components::{ConfirmDialog, Message, PencilIcon, TrashIcon};

use super::edit_wg_modal::EditWatchGroupModal;

#[component]
pub fn WatchGroupAssignment(
    assignment: ClientWatchGroupDto,
    client_id: String,
    on_changed: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let wg_id = assignment.server_watch_group_id;
    let wg_name = assignment.server_watch_group_name;
    let initial_path = assignment.path_to_monitor;
    let initial_exclude_dirs = assignment.exclude_dirs.join("\n");
    let initial_exclude_dot = assignment.exclude_dot_dirs;

    let client_id_sv = StoredValue::new(client_id);
    let on_changed_sv = StoredValue::new(on_changed);

    let confirm_delete = RwSignal::new(false);
    let show_edit_modal = RwSignal::new(false);
    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    let do_delete = move || {
        let id = client_id_sv.get_value();
        spawn_local(async move {
            match api::delete_client_watch_group(&id, wg_id).await {
                Ok(()) => on_changed_sv.get_value()(),
                Err(e) => set_msg.set(Some((false, format!("Error: {e}")))),
            }
        });
    };

    let confirm_msg = format!("Delete '{}' assignment?", wg_name);
    let exclude_dirs_display = if initial_exclude_dirs.is_empty() {
        "—".to_string()
    } else {
        initial_exclude_dirs.replace('\n', ", ")
    };
    let path_for_display = initial_path.clone();

    view! {
        <div class="wg-assignment">
            <div class="flex-between">
                <span class="font-semibold">
                    {wg_name}
                    " "
                    <span class="text-muted text-xs">"(#" {wg_id} ")"</span>
                </span>
                <div class="flex gap-1">
                    <button
                        class="btn btn-icon btn-primary"
                        title="Edit"
                        on:click=move |_| show_edit_modal.set(true)
                    >
                        <PencilIcon/>
                    </button>
                    <button
                        class="btn btn-icon btn-danger"
                        title="Delete"
                        on:click=move |_| confirm_delete.set(true)
                    >
                        <TrashIcon/>
                    </button>
                </div>
            </div>

            <div class="detail-grid" style="margin-top: 0.5rem; font-size: 0.85rem;">
                <span class="detail-label">"Path"</span>
                <span class="detail-value">{path_for_display}</span>
                <span class="detail-label">"Exclude dirs"</span>
                <span class="detail-value">{exclude_dirs_display}</span>
                <span class="detail-label">"Exclude dots"</span>
                <span class="detail-value">{if initial_exclude_dot { "yes" } else { "no" }}</span>
            </div>

            <Message signal=msg />

            <EditWatchGroupModal
                show=show_edit_modal
                client_id=client_id_sv.get_value()
                wg_id=wg_id
                initial_path=initial_path
                initial_exclude_dirs=initial_exclude_dirs
                initial_exclude_dot=initial_exclude_dot
                on_saved=move || on_changed_sv.get_value()()
            />

            <ConfirmDialog show=confirm_delete message=confirm_msg on_confirm=do_delete />
        </div>
    }
}
