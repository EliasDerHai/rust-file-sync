use leptos::prelude::*;
use leptos_router::components::A;

use crate::api;
use crate::components::{Card, EmptyState, Loading};

#[component]
pub fn ConfigsPage() -> impl IntoView {
    let configs = LocalResource::new(api::fetch_configs);

    view! {
        <div class="container">
            <h1>"Client Configs"</h1>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match configs.await {
                        Ok(clients) => {
                            if clients.is_empty() {
                                view! { <EmptyState message="No clients registered yet." /> }.into_any()
                            } else {
                                view! {
                                    <ul style="list-style: none; padding: 0;">
                                        {clients.into_iter().map(|client| {
                                            let id = client.id.clone();
                                            let edit_href = format!("/config/{}", id);
                                            view! {
                                                <li>
                                                    <Card>
                                                        <div class="flex-between" style="margin-bottom: 1rem;">
                                                            <div>
                                                                <div class="text-lg font-semibold">{client.host_name}</div>
                                                                <div class="text-xs text-muted text-mono">{client.id}</div>
                                                            </div>
                                                            <A href=edit_href attr:class="btn btn-primary">"Edit"</A>
                                                        </div>
                                                        <div class="detail-grid">
                                                            <span class="detail-label">"Path:"</span>
                                                            <span class="detail-value">{client.path_to_monitor}</span>
                                                            <span class="detail-label">"Group:"</span>
                                                            <span class="detail-value">{client.server_watch_group_name}</span>
                                                            <span class="detail-label">"Poll interval:"</span>
                                                            <span class="detail-value">{format!("{}ms", client.min_poll_interval_in_ms)}</span>
                                                            <span class="detail-label">"Exclude dirs:"</span>
                                                            <span class="detail-value">
                                                                {if client.exclude_dirs.is_empty() {
                                                                    "none".to_string()
                                                                } else {
                                                                    client.exclude_dirs.join(", ")
                                                                }}
                                                            </span>
                                                            <span class="detail-label">"Exclude dot dirs:"</span>
                                                            <span class="detail-value">{client.exclude_dot_dirs.to_string()}</span>
                                                        </div>
                                                    </Card>
                                                </li>
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
