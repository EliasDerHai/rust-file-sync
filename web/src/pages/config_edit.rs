use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use crate::api;
use crate::components::{Card, Loading, Message};
use crate::types::AdminConfigUpdateDto;

#[component]
pub fn ConfigEditPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id");

    let client_data = LocalResource::new(move || {
        let id = id().unwrap_or_default();
        async move { api::fetch_config(&id).await }
    });

    let watch_groups_data = LocalResource::new(api::fetch_watch_groups);

    let (msg, set_msg) = signal::<Option<(bool, String)>>(None);

    view! {
        <div class="container-narrow">
            <A href="/app/configs" attr:class="back-link">"← Back to list"</A>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    let client_result = client_data.await;
                    let groups_result = watch_groups_data.await;

                    match (client_result, groups_result) {
                        (Ok(client), Ok(groups)) => {
                            let client_id = client.id.clone();
                            let path = RwSignal::new(client.path_to_monitor.clone());
                            let poll_interval = RwSignal::new(client.min_poll_interval_in_ms.to_string());
                            let watch_group_id = RwSignal::new(client.server_watch_group_id.to_string());
                            let exclude_dirs_text = RwSignal::new(client.exclude_dirs.join("\n"));
                            let exclude_dot = RwSignal::new(client.exclude_dot_dirs);

                            let on_save = move |_| {
                                let id = client_id.clone();
                                let dto = AdminConfigUpdateDto {
                                    path_to_monitor: path.get(),
                                    min_poll_interval_in_ms: poll_interval.get().parse().unwrap_or(1000),
                                    exclude_dirs: exclude_dirs_text.get()
                                        .lines()
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect(),
                                    exclude_dot_dirs: exclude_dot.get(),
                                    server_watch_group_id: watch_group_id.get().parse().unwrap_or(1),
                                };
                                let set_msg = set_msg;
                                leptos::task::spawn_local(async move {
                                    match api::update_config(&id, &dto).await {
                                        Ok(text) => set_msg.set(Some((true, text))),
                                        Err(e) => set_msg.set(Some((false, format!("Error: {}", e)))),
                                    }
                                });
                            };

                            view! {
                                <h1>{client.host_name}</h1>
                                <div class="text-xs text-muted text-mono" style="margin-bottom: 2rem;">{client.id}</div>

                                <Message signal=msg />

                                <Card>
                                    <div class="form-group">
                                        <label for="path">"Path to monitor"</label>
                                        <input type="text" id="path" class="form-input"
                                            bind:value=path
                                        />
                                    </div>

                                    <div class="form-group">
                                        <label for="poll">"Poll interval (ms)"</label>
                                        <input type="number" id="poll" class="form-input" min="100"
                                            bind:value=poll_interval
                                        />
                                    </div>

                                    <div class="form-group">
                                        <label for="group">"Server Watch Group"</label>
                                        <select id="group" class="form-input"
                                            bind:value=watch_group_id
                                        >
                                            {groups.into_iter().map(|g| {
                                                let val = g.id.to_string();
                                                view! {
                                                    <option value=val>{g.name}</option>
                                                }
                                            }).collect_view()}
                                        </select>
                                        <div class="help-text">"Select which server watch group this client belongs to"</div>
                                    </div>

                                    <div class="form-group">
                                        <label for="exclude">"Exclude directories"</label>
                                        <textarea id="exclude" class="form-input" rows="3"
                                            placeholder="One per line"
                                            bind:value=exclude_dirs_text
                                        ></textarea>
                                        <div class="help-text">"One directory per line (e.g., node_modules, .git)"</div>
                                    </div>

                                    <div class="form-group">
                                        <div class="checkbox-group">
                                            <input type="checkbox" id="dot"
                                                bind:checked=exclude_dot
                                            />
                                            <label for="dot">"Exclude dot directories"</label>
                                        </div>
                                        <div class="help-text">"Automatically exclude directories starting with a dot"</div>
                                    </div>

                                    <div class="btn-group">
                                        <button class="btn btn-primary btn-lg" on:click=on_save>"Save Changes"</button>
                                        <A href="/app/configs" attr:class="btn btn-secondary btn-lg">"Cancel"</A>
                                    </div>
                                </Card>
                            }.into_any()
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            view! { <div class="message message-error">"Error: " {e}</div> }.into_any()
                        }
                    }
                })}
            </Suspense>
        </div>
    }
}
