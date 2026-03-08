use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::{ClientDto, ServerWatchGroup};

use crate::api;
use crate::components::{
    Card, ConfirmDialog, Loading, Message, PencilIcon, ToastSignal, TrashIcon,
};

use super::add_watch_group_form::AddWatchGroupForm;
use super::edit_client_modal::EditClientModal;
use super::watch_group_assignment::WatchGroupAssignment;

#[component]
pub fn ClientCard(
    client: ClientDto,
    server_watch_groups: Vec<ServerWatchGroup>,
    on_changed: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client.id);
    let host_name = client.host_name;
    let current_poll_ms = client.min_poll_interval_in_ms;
    let server_watch_groups = StoredValue::new(server_watch_groups);
    let on_changed_sv = StoredValue::new(on_changed);

    let confirm_delete = RwSignal::new(false);
    let show_edit_modal = RwSignal::new(false);
    let watch_group_trigger = RwSignal::new(0u32);
    let msg = ToastSignal::new();

    let watch_groups = LocalResource::new(move || {
        watch_group_trigger.get();
        let id = client_id.get_value();
        async move { api::fetch_client_watch_groups(&id).await }
    });

    let do_delete = move || {
        let id = client_id.get_value();
        spawn_local(async move {
            match api::delete_client(&id).await {
                Ok(()) => on_changed_sv.get_value()(),
                Err(e) => msg.error(e),
            }
        });
    };

    let confirm_msg = format!("Delete client '{}'?", host_name);

    view! {
        <li>
            <Card>
                <div class="flex-between">
                    <span class="text-lg font-semibold">{host_name}</span>
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

                <div class="detail-grid" style="margin-top: 0.75rem;">
                    <span class="detail-label">"ID"</span>
                    <span class="detail-value text-xs">{move || client_id.get_value()}</span>
                    <span class="detail-label">"Poll interval"</span>
                    <span class="detail-value">{current_poll_ms}"ms"</span>
                </div>

                <Message signal=msg />

                <div class="watch-group-list">
                    <Suspense fallback=Loading>
                        {move || Suspend::new(async move {
                            match watch_groups.await {
                                Ok(watch_group_list) => {
                                    let svr_watch_groups = server_watch_groups.get_value();
                                    view! {
                                        {watch_group_list.into_iter().map(|watch_group| {
                                            view! {
                                                <WatchGroupAssignment
                                                    assignment=watch_group
                                                    client_id=client_id.get_value()
                                                    on_changed=move || watch_group_trigger.update(|t| *t += 1)
                                                />
                                            }
                                        }).collect_view()}
                                        <AddWatchGroupForm
                                            client_id=client_id.get_value()
                                            server_watch_groups=svr_watch_groups
                                            on_created=move || watch_group_trigger.update(|t| *t += 1)
                                        />
                                    }.into_any()
                                }
                                Err(e) => view! {
                                    <div class="message message-error">
                                        "Error loading watch groups: " {e}
                                    </div>
                                }.into_any(),
                            }
                        })}
                    </Suspense>
                </div>

                <EditClientModal
                    show=show_edit_modal
                    client_id=client_id.get_value()
                    current_poll_ms=current_poll_ms
                    on_saved=move || on_changed_sv.get_value()()
                />

                <ConfirmDialog show=confirm_delete message=confirm_msg on_confirm=do_delete />
            </Card>
        </li>
    }
}
