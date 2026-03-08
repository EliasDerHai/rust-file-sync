use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::{ClientDto, ServerWatchGroup};

use crate::api;
use crate::components::{Card, ConfirmDialog, Loading, Message, PencilIcon, TrashIcon};

use super::add_wg_form::AddWatchGroupForm;
use super::edit_client_modal::EditClientModal;
use super::wg_assignment::WatchGroupAssignment;

#[component]
pub fn ClientCard(
    client: ClientDto,
    server_wgs: Vec<ServerWatchGroup>,
    on_changed: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client.id);
    let host_name = client.host_name;
    let current_poll_ms = client.min_poll_interval_in_ms;
    let server_wgs = StoredValue::new(server_wgs);
    let on_changed_sv = StoredValue::new(on_changed);

    let confirm_delete = RwSignal::new(false);
    let show_edit_modal = RwSignal::new(false);
    let wg_trigger = RwSignal::new(0u32);
    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    let watch_groups = LocalResource::new(move || {
        wg_trigger.get();
        let id = client_id.get_value();
        async move { api::fetch_client_watch_groups(&id).await }
    });

    let do_delete = move || {
        let id = client_id.get_value();
        spawn_local(async move {
            match api::delete_client(&id).await {
                Ok(()) => on_changed_sv.get_value()(),
                Err(e) => set_msg.set(Some((false, format!("Error: {e}")))),
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
                                Ok(wg_list) => {
                                    let svr_wgs = server_wgs.get_value();
                                    view! {
                                        {wg_list.into_iter().map(|wg| {
                                            view! {
                                                <WatchGroupAssignment
                                                    assignment=wg
                                                    client_id=client_id.get_value()
                                                    on_changed=move || wg_trigger.update(|t| *t += 1)
                                                />
                                            }
                                        }).collect_view()}
                                        <AddWatchGroupForm
                                            client_id=client_id.get_value()
                                            server_wgs=svr_wgs
                                            on_created=move || wg_trigger.update(|t| *t += 1)
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
