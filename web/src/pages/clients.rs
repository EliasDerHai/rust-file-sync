use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::dtos::{
    ClientDto, ClientUpdateDto, ClientWatchGroupCreateDto, ClientWatchGroupDto,
    ClientWatchGroupUpdateDto, ServerWatchGroup,
};

use crate::api;
use crate::components::{Card, ConfirmDialog, EmptyState, Loading, Message};

#[component]
pub fn ClientsPage() -> impl IntoView {
    let (trigger, set_trigger) = signal(0u32);
    let clients = LocalResource::new(move || {
        trigger.get();
        api::fetch_clients()
    });
    let server_wgs = LocalResource::new(api::fetch_watch_groups);

    view! {
        <div class="container">
            <h1>"Clients"</h1>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    let clients_res = clients.await;
                    let wgs_res = server_wgs.await;
                    match (clients_res, wgs_res) {
                        (Err(e), _) => view! {
                            <div class="message message-error">"Error loading clients: " {e}</div>
                        }.into_any(),
                        (_, Err(e)) => view! {
                            <div class="message message-error">"Error loading watch groups: " {e}</div>
                        }.into_any(),
                        (Ok(client_list), Ok(wg_list)) => {
                            if client_list.is_empty() {
                                view! { <EmptyState message="No clients registered yet." /> }.into_any()
                            } else {
                                view! {
                                    <ul style="list-style: none; padding: 0;">
                                        {client_list.into_iter().map(|client| {
                                            let wg_list = wg_list.clone();
                                            view! {
                                                <ClientCard
                                                    client=client
                                                    server_wgs=wg_list
                                                    on_client_deleted=move || set_trigger.update(|t| *t += 1)
                                                />
                                            }
                                        }).collect_view()}
                                    </ul>
                                }.into_any()
                            }
                        }
                    }
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn ClientCard(
    client: ClientDto,
    server_wgs: Vec<ServerWatchGroup>,
    on_client_deleted: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client.id);
    let host_name = client.host_name;
    let server_wgs = StoredValue::new(server_wgs);
    // Store callback so do_delete is Fn + Clone + Send + Sync
    let on_client_deleted_sv = StoredValue::new(on_client_deleted);

    let confirm_delete = RwSignal::new(false);
    let edit_poll = RwSignal::new(false);
    let poll_value = RwSignal::new(client.min_poll_interval_in_ms.to_string());
    let wg_trigger = RwSignal::new(0u32);
    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    let watch_groups = LocalResource::new(move || {
        wg_trigger.get();
        let id = client_id.get_value();
        async move { api::fetch_client_watch_groups(&id).await }
    });

    // All captures are Copy → do_delete is Copy + Fn + Clone + Send + Sync
    let do_delete = move || {
        let id = client_id.get_value();
        spawn_local(async move {
            match api::delete_client(&id).await {
                Ok(()) => on_client_deleted_sv.get_value()(),
                Err(e) => set_msg.set(Some((false, format!("Error: {e}")))),
            }
        });
    };

    // All captures are Copy → do_save_poll is Copy
    let do_save_poll = move |_| {
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
                    edit_poll.set(false);
                    set_msg.set(Some((true, "Saved!".to_string())));
                }
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
                        <Show when=move || !edit_poll.get()>
                            <button class="btn btn-primary" on:click=move |_| edit_poll.set(true)>
                                "Edit"
                            </button>
                        </Show>
                        <button class="btn btn-danger" on:click=move |_| confirm_delete.set(true)>
                            "Delete"
                        </button>
                    </div>
                </div>

                <Show when=move || !edit_poll.get()>
                    <div class="detail-grid" style="margin-top: 0.75rem;">
                        <span class="detail-label">"ID"</span>
                        <span class="detail-value text-xs">{move || client_id.get_value()}</span>
                        <span class="detail-label">"Poll interval"</span>
                        <span class="detail-value">{move || poll_value.get()}"ms"</span>
                    </div>
                </Show>
                <Show when=move || edit_poll.get()>
                    <div class="detail-grid" style="margin-top: 0.75rem;">
                        <span class="detail-label">"ID"</span>
                        <span class="detail-value text-xs">{move || client_id.get_value()}</span>
                        <span class="detail-label">"Poll interval (ms)"</span>
                        <div class="flex gap-1">
                            <input
                                type="number"
                                class="form-input"
                                style="width: 120px;"
                                bind:value=poll_value
                            />
                            <button class="btn btn-success" on:click=do_save_poll>
                                "Save"
                            </button>
                            <button
                                class="btn btn-secondary"
                                on:click=move |_| edit_poll.set(false)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                </Show>

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

                <ConfirmDialog show=confirm_delete message=confirm_msg on_confirm=do_delete />
            </Card>
        </li>
    }
}

#[component]
fn WatchGroupAssignment(
    assignment: ClientWatchGroupDto,
    client_id: String,
    on_changed: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let wg_id = assignment.server_watch_group_id;
    let wg_name = assignment.server_watch_group_name;
    let client_id = StoredValue::new(client_id);
    // Store both callback uses so do_delete and do_save become Copy
    let on_changed_del = StoredValue::new(on_changed.clone());
    let on_changed_save = StoredValue::new(on_changed);

    let confirm_delete = RwSignal::new(false);
    let editing = RwSignal::new(false);
    let path = RwSignal::new(assignment.path_to_monitor);
    let exclude_dirs_text = RwSignal::new(assignment.exclude_dirs.join("\n"));
    let exclude_dot = RwSignal::new(assignment.exclude_dot_dirs);
    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    // All captures Copy → do_delete is Copy + Fn + Clone + Send + Sync
    let do_delete = move || {
        let id = client_id.get_value();
        spawn_local(async move {
            match api::delete_client_watch_group(&id, wg_id).await {
                Ok(()) => on_changed_del.get_value()(),
                Err(e) => set_msg.set(Some((false, format!("Error: {e}")))),
            }
        });
    };

    // All captures Copy → do_save is Copy
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
                    editing.set(false);
                    on_changed_save.get_value()();
                }
                Err(e) => set_msg.set(Some((false, format!("Error: {e}")))),
            }
        });
    };

    let confirm_msg = format!("Delete '{}' assignment?", wg_name);

    view! {
        <div class="wg-assignment">
            <div class="flex-between">
                <span class="font-semibold">
                    {wg_name}
                    " "
                    <span class="text-muted text-xs">"(#" {wg_id} ")"</span>
                </span>
                <div class="flex gap-1">
                    <Show when=move || !editing.get()>
                        <button class="btn btn-primary" on:click=move |_| editing.set(true)>
                            "Edit"
                        </button>
                        <button class="btn btn-danger" on:click=move |_| confirm_delete.set(true)>
                            "Delete"
                        </button>
                    </Show>
                    <Show when=move || editing.get()>
                        <button class="btn btn-success" on:click=do_save>
                            "Save"
                        </button>
                        <button class="btn btn-secondary" on:click=move |_| editing.set(false)>
                            "Cancel"
                        </button>
                    </Show>
                </div>
            </div>

            <Show when=move || !editing.get()>
                <div class="detail-grid" style="margin-top: 0.5rem; font-size: 0.85rem;">
                    <span class="detail-label">"Path"</span>
                    <span class="detail-value">{move || path.get()}</span>
                    <span class="detail-label">"Exclude dirs"</span>
                    <span class="detail-value">
                        {move || {
                            let dirs = exclude_dirs_text.get();
                            if dirs.is_empty() {
                                "—".to_string()
                            } else {
                                dirs.replace('\n', ", ")
                            }
                        }}
                    </span>
                    <span class="detail-label">"Exclude dots"</span>
                    <span class="detail-value">
                        {move || if exclude_dot.get() { "yes" } else { "no" }}
                    </span>
                </div>
            </Show>
            <Show when=move || editing.get()>
                <div class="form-group" style="margin-top: 0.5rem;">
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
            </Show>

            <Message signal=msg />

            <ConfirmDialog show=confirm_delete message=confirm_msg on_confirm=do_delete />
        </div>
    }
}

#[component]
fn AddWatchGroupForm(
    client_id: String,
    server_wgs: Vec<ServerWatchGroup>,
    on_created: impl Fn() + 'static + Clone + Send + Sync,
) -> impl IntoView {
    let client_id = StoredValue::new(client_id);
    // Store callback and wg list so do_add and select options are Copy inside <Show>
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

    // All captures Copy → do_add is Copy
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
