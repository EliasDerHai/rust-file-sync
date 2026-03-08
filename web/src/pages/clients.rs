use leptos::prelude::*;

use crate::api;
use crate::components::{Card, EmptyState, Loading};

#[component]
pub fn ClientsPage() -> impl IntoView {
    let (trigger, set_trigger) = signal(0u32);

    let clients = LocalResource::new(move || {
        trigger.get();
        api::fetch_clients()
    });

    let on_refresh = move |_| {
        set_trigger.update(|x| {
            *x += 1;
        });
    };

    view! {
        <div class="container">
            <h1>"Clients"</h1>
            <button class="btn btn-primary" on:click=on_refresh>"Refresh"</button>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match clients.await {
                        Ok(clients) => {
                            if clients.is_empty() {
                                view! { <EmptyState message="No clients registered yet." /> }.into_any()
                            } else {
                                view! {
                                    <ul style="list-style: none; padding: 0;">
                                        {clients.into_iter().map(|client| {
                                            view! {
                                                <Card>
                                                    <div class="text-lg font-semibold" style="margin-bottom: 0.75rem;">{client.host_name}</div>
                                                    <div class="detail-grid">
                                                        <span class="detail-label">"ID"</span>
                                                        <span class="detail-value text-xs">{client.id}</span>
                                                        <span class="detail-label">"Poll interval"</span>
                                                        <span class="detail-value">{client.min_poll_interval_in_ms}"ms"</span>
                                                    </div>
                                                </Card>
                                            }
                                        }).collect_view()}
                                    </ul>
                                }.into_any()
                            }
                        }
                        Err(e) => view! { <div class="message message-error">"Error: " {e}</div> }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}
