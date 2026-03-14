use crate::api;
use crate::components::{
    EmptyState, Loading, Message, PencilIcon, TagBadge, ToastSignal, TrashIcon,
};
use itertools::Itertools;
use leptos::prelude::*;
use shared::dtos::LinkDto;

#[component]
pub fn LinksPage() -> impl IntoView {
    let signal = RwSignal::new(0u32);
    let links = LocalResource::new(move || {
        signal.get();
        api::fetch_links()
    });
    let msg = ToastSignal::new();
    let selected_tags: RwSignal<Vec<String>> = RwSignal::new(vec![]);

    // Single Callback<String> — toggles a tag in/out of selected_tags.
    // Callback is Copy, so it can be passed to every Link without cloning.
    let badge_click = Callback::new(move |t: String| {
        selected_tags.update(|v| {
            if v.contains(&t) {
                v.retain(|x| x != &t);
            } else {
                v.push(t);
            }
        });
    });

    view! {
        <div class="container">
            <h1>"Links"</h1>
            <Message signal=msg />

            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match links.await {
                        Err(e) => view! { <div class="message message-error">"Error: " {e}</div> }.into_any(),
                        Ok(links) => {
                            if links.is_empty() {
                                view! { <EmptyState message="No links saved yet." /> }.into_any()
                            } else {
                                let unique_tags: Vec<String> = {
                                    let mut seen = std::collections::HashSet::new();
                                    links.iter()
                                        .flat_map(|l| l.tags.iter().cloned())
                                        .filter(|t| seen.insert(t.clone()))
                                        .collect()
                                };
                                let links = StoredValue::new(links);

                                view! {
                                    {(!unique_tags.is_empty()).then(|| view! {
                                        <div class="tag-filter">
                                            {unique_tags.into_iter().map(|tag| {
                                                let tag_for_active = tag.clone();
                                                let tag_for_click = tag.clone();
                                                view! {
                                                    <TagBadge
                                                        tag=tag
                                                        active=Signal::derive(move || selected_tags.get().contains(&tag_for_active))
                                                        on_click=Callback::new(move |_| badge_click.run(tag_for_click.clone()))
                                                    />
                                                }
                                            }).collect_view()}
                                        </div>
                                    })}

                                    <ul style="list-style: none; padding: 0;">
                                        {move || {
                                            let active = selected_tags.get();
                                            links.get_value()
                                                .into_iter()
                                                .filter(|l| active.is_empty() || active.iter().all(|t| l.tags.contains(t)))
                                                .sorted_by_key(|l| l.created_at)
                                                .enumerate()
                                                .map(|(i, link)| view! {
                                                    <Link i link selected_tags badge_click signal />
                                                })
                                                .collect_view()
                                        }}
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
pub fn Link(
    i: usize,
    link: LinkDto,
    selected_tags: RwSignal<Vec<String>>,
    badge_click: Callback<String>,
    signal: RwSignal<u32>,
) -> impl IntoView {
    view! {
        <div class={odd(i)}>
            <div>
                <a href={link.url.clone()} target="_blank" style="padding-right: 1em">{link.link_text(30)}</a>
                {link.tags.iter().map(|t| {
                    let tag = t.clone();
                    let t_active = t.clone();
                    let t_click = t.clone();
                    view! {
                        <TagBadge
                            tag
                            active=Signal::derive(move || selected_tags.get().contains(&t_active))
                            on_click=Callback::new(move |_| badge_click.run(t_click.clone()))
                        />
                    }
                }).collect_view()}
            </div>
            <div style="display: flex; gap: 5px">
                <button
                    class="btn btn-icon btn-primary"
                    title="Edit"
                    on:click=move |_| println!("edit")
                >
                    <PencilIcon/>
                </button>
                <button
                    class="btn btn-icon btn-danger"
                    title="Delete"
                    on:click=move |_| println!("delete")
                >
                    <TrashIcon/>
                </button>
            </div>
        </div>
    }.into_any()
}

fn odd(i: usize) -> &'static str {
    match i.is_multiple_of(2) {
        true => "link",
        false => "link odd",
    }
}
