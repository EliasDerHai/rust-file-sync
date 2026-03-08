mod add_watch_group_form;
mod client_card;
mod edit_client_modal;
mod edit_watch_group_modal;
mod watch_group_assignment;

use client_card::ClientCard;

use crate::api;
use crate::components::{EmptyState, Loading};
use leptos::prelude::*;

#[component]
pub fn ClientsPage() -> impl IntoView {
    let (trigger, set_trigger) = signal(0u32);
    let clients = LocalResource::new(move || {
        trigger.get();
        api::fetch_clients()
    });
    let server_watch_groups = LocalResource::new(api::fetch_watch_groups);

    view! {
        <div class="container">
            <h1>"Clients"</h1>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    let clients_res = clients.await;
                    let watch_groups_res = server_watch_groups.await;
                    match (clients_res, watch_groups_res) {
                        (Err(e), _) => view! {
                            <div class="message message-error">"Error loading clients: " {e}</div>
                        }.into_any(),
                        (_, Err(e)) => view! {
                            <div class="message message-error">"Error loading watch groups: " {e}</div>
                        }.into_any(),
                        (Ok(clients), Ok(watch_groups)) => {
                            if clients.is_empty() {
                                view! { <EmptyState message="No clients registered yet." /> }.into_any()
                            } else {
                                view! {
                                    <ul style="list-style: none; padding: 0;">
                                        {clients.into_iter().map(|client| {
                                            let watch_group_list = watch_groups.clone();
                                            view! {
                                                <ClientCard
                                                    client=client
                                                    server_watch_groups=watch_group_list
                                                    on_changed=move || set_trigger.update(|t| *t += 1)
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
