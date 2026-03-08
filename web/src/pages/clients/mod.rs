mod add_wg_form;
mod client_card;
mod edit_client_modal;
mod edit_wg_modal;
mod wg_assignment;

use client_card::ClientCard;

use leptos::prelude::*;
use crate::api;
use crate::components::{EmptyState, Loading};

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
