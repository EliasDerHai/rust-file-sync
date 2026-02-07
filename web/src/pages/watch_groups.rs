use leptos::prelude::*;

use crate::api;
use crate::components::{Card, EmptyState, Loading, Message};
use crate::types::WatchGroupNameDto;

#[component]
pub fn WatchGroupsPage() -> impl IntoView {
    let (trigger, set_trigger) = signal(0u32);
    let groups = LocalResource::new(move || {
        trigger.get();
        api::fetch_watch_groups()
    });

    let new_name = RwSignal::new(String::new());
    let (create_msg, set_create_msg) = signal::<Option<(bool, String)>>(None);

    let on_create = move |_| {
        let name = new_name.get().trim().to_string();
        if name.is_empty() {
            return;
        }
        let dto = WatchGroupNameDto { name };
        leptos::task::spawn_local(async move {
            match api::create_watch_group(&dto).await {
                Ok(_) => {
                    new_name.set(String::new());
                    set_trigger.update(|t| *t += 1);
                }
                Err(e) => set_create_msg.set(Some((false, format!("Error: {}", e)))),
            }
        });
    };

    view! {
        <div class="container">
            <h1>"Watch Groups"</h1>

            <Card dashed=true>
                <div class="flex gap-2">
                    <input type="text" class="form-input" placeholder="New watch group name"
                        style="flex: 1;"
                        bind:value=new_name
                    />
                    <button class="btn btn-success" on:click=on_create>"Create"</button>
                </div>
            </Card>
            <Message signal=create_msg />

            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match groups.await {
                        Ok(group_list) => {
                            if group_list.is_empty() {
                                view! { <EmptyState message="No watch groups configured yet." /> }.into_any()
                            } else {
                                view! {
                                    <ul style="list-style: none; padding: 0;">
                                        {group_list.into_iter().map(|group| {
                                            view! { <WatchGroupCard group_id=group.id group_name=group.name.clone() set_trigger /> }
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

#[component]
fn WatchGroupCard(
    group_id: i64,
    group_name: String,
    set_trigger: WriteSignal<u32>,
) -> impl IntoView {
    let editing = RwSignal::new(false);
    let edit_name = RwSignal::new(group_name.clone());
    let display_name = RwSignal::new(group_name);
    let (card_msg, set_card_msg) = signal::<Option<(bool, String)>>(None);

    let on_edit = move |_| {
        edit_name.set(display_name.get());
        editing.set(true);
    };

    let on_cancel = move |_| {
        editing.set(false);
    };

    let on_save = move |_| {
        let dto = WatchGroupNameDto {
            name: edit_name.get(),
        };
        leptos::task::spawn_local(async move {
            match api::update_watch_group(group_id, &dto).await {
                Ok(_) => {
                    display_name.set(edit_name.get());
                    editing.set(false);
                    set_card_msg.set(Some((true, "Saved!".to_string())));
                    set_trigger.update(|t| *t += 1);
                    // Auto-dismiss after 3s
                    gloo_timers::callback::Timeout::new(3_000, move || {
                        set_card_msg.set(None);
                    })
                    .forget();
                }
                Err(e) => {
                    set_card_msg.set(Some((false, format!("Error: {}", e))));
                }
            }
        });
    };

    view! {
        <li>
            <Card>
                <div class="flex-between">
                    <div>
                        <Show when=move || !editing.get()>
                            <span class="text-lg font-semibold">{move || display_name.get()}</span>
                        </Show>
                        <Show when=move || editing.get()>
                            <input type="text" class="name-input"
                                bind:value=edit_name
                            />
                        </Show>
                        <div class="text-xs text-muted">"ID: " {group_id}</div>
                    </div>
                    <div class="flex gap-1">
                        <Show when=move || !editing.get()>
                            <button class="btn btn-primary" on:click=on_edit>"Edit"</button>
                        </Show>
                        <Show when=move || editing.get()>
                            <button class="btn btn-success" on:click=on_save>"Save"</button>
                            <button class="btn btn-secondary" on:click=on_cancel>"Cancel"</button>
                        </Show>
                    </div>
                </div>
                <Message signal=card_msg />
            </Card>
        </li>
    }
}
