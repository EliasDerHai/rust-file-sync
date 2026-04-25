use crate::api;
use crate::components::{
    EmptyState, Loading, Message, Modal, PencilIcon, PlusIcon, TagBadge, ToastSignal, TrashIcon,
};
use itertools::Itertools;
use leptos::prelude::*;
use leptos::reactive::spawn_local;
use shared::dtos::{LinkCreateDto, LinkDto};

#[component]
pub fn LinksPage() -> impl IntoView {
    let show_add = RwSignal::new(false);
    let reload_links = RwSignal::new(0u32);
    let links = LocalResource::new(move || {
        reload_links.get();
        api::fetch_links()
    });
    let msg = ToastSignal::new();
    let selected_tags: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let distinct_tags: RwSignal<Vec<String>> = RwSignal::new(vec![]);
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
                            let tags: Vec<String> = {
                                let mut seen = std::collections::HashSet::new();
                                links.iter()
                                    .flat_map(|l| l.tags.iter().cloned())
                                    .filter(|t| seen.insert(t.clone()))
                                    .collect()
                            };
                            distinct_tags.set(tags.clone());

                            if links.is_empty() {
                                view! { <EmptyState message="No links saved yet." /> }.into_any()
                            } else {
                                let links = StoredValue::new(links);

                                view! {
                                    {(!tags.is_empty()).then(|| view! {
                                        <div class="tag-filter">
                                            {tags.into_iter().map(|tag| {
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
                                                    <Link i link selected_tags badge_click reload_links distinct_tags=distinct_tags.into() />
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

            <AddOrEditLinkModal show=show_add distinct_tags=Signal::from(distinct_tags) on_saved=move||reload_links.update(|v| *v += 1) />
            <button
                class="btn btn-icon btn-primary"
                title="Add"
                on:click=move |_| show_add.set(true)
            >
                <PlusIcon/>
            </button>
        </div>
    }
}

#[component]
pub fn Link(
    i: usize,
    link: LinkDto,
    selected_tags: RwSignal<Vec<String>>,
    badge_click: Callback<String>,
    reload_links: RwSignal<u32>,
    distinct_tags: Signal<Vec<String>>,
) -> impl IntoView {
    let show_edit = RwSignal::new(false);
    view! {
        <div class={odd(i)}>
            <div>
                <a href={link.url.clone()} target="_blank" style="padding-right: 1em">{link.link_text(30)}</a>
                {link.tags.clone().into_iter().map(|tag| {
                    let compare_tag = tag.clone();
                    view! {
                        <TagBadge
                            tag=tag.clone()
                            active=Signal::derive(move || selected_tags.get().contains(&compare_tag))
                            on_click=Callback::new(move |_| badge_click.run(tag.clone()))
                        />
                    }
                }).collect_view()}
            </div>
            <div style="display: flex; gap: 5px">

                <AddOrEditLinkModal show=show_edit distinct_tags on_saved=move||reload_links.update(|v| *v += 1) val=link.clone() />

                <button
                    class="btn btn-icon btn-primary"
                    title="Edit"
                    on:click=move |_| {
                        println!("edit");
                        show_edit.set(!show_edit.get());
                    }
                >
                    <PencilIcon/>
                </button>
                <button
                    class="btn btn-icon btn-danger"
                    title="Delete"
                    on:click=move |_| {
                        let url = link.url.clone();
                        spawn_local(async move {
                            if api::delete_link(&url).await.is_ok() {
                                reload_links.update(|n| *n += 1);
                            }
                        });
                    }
                >
                    <TrashIcon/>
                </button>
            </div>
        </div>
    }.into_any()
}

#[component]
pub fn AddOrEditLinkModal(
    show: RwSignal<bool>,
    distinct_tags: Signal<Vec<String>>,
    on_saved: impl Fn() + 'static + Clone + Send + Sync,
    #[prop(optional)] val: Option<LinkDto>,
) -> impl IntoView {
    let (url, title, initial_tags) = match val {
        Some(LinkDto {
            created_at: _,
            url,
            title,
            tags,
        }) => (url, title.unwrap_or_default(), tags),
        None => (String::new(), String::new(), vec![]),
    };
    let url = RwSignal::new(url);
    let title = RwSignal::new(title);
    let tags = RwSignal::new("".to_string());
    let selected_tags: RwSignal<Vec<String>> = RwSignal::new(initial_tags);

    let badge_click = Callback::new(move |t: String| {
        selected_tags.update(|v| {
            if v.contains(&t) {
                v.retain(|x| x != &t);
            } else {
                v.push(t);
            }
        });
    });

    let on_save = move || {
        let url = url.get();
        let title = title.get();
        let title = if title.is_empty() { None } else { Some(title) };

        let dto = LinkCreateDto { url, title };

        async move { api::create_link(dto).await }
    };

    let commit_tags = move || {
        let raw = tags.get();
        let new_tags: Vec<String> = raw
            .split_whitespace()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !new_tags.is_empty() {
            selected_tags.update(|v| {
                for t in new_tags {
                    if !v.contains(&t) {
                        v.push(t);
                    }
                }
            });
            tags.set(String::new());
        }
    };

    view! {
        <Modal show title="Add link" on_save on_saved>
            <div class="form-group">
                <label>"Url"</label>
                <input type="text" class="form-input" bind:value=url/>
            </div>
            <div class="form-group">
                <label>"Title"</label>
                <input type="text" class="form-input" bind:value=title/>
            </div>

            {move || distinct_tags.get().into_iter().map(|tag| {
                    let compare_tag = tag.clone();
                    view! {
                        <TagBadge
                            tag=tag.clone()
                            active=Signal::derive(move || selected_tags.get().contains(&compare_tag))
                            on_click=Callback::new(move |_| badge_click.run(tag.clone()))
                        />
                    }
           }).collect_view()}

            <div class="form-group">
                <label>"Tags"</label>
                <div style="display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 4px;">
                    {move || selected_tags.get().into_iter().map(|tag| {
                        view! {
                            <span style="display: inline-flex; align-items: center; gap: 4px; background: var(--badge-bg, #e0e0e0); padding: 2px 6px; border-radius: 4px; font-size: 0.85em;">
                                {tag.clone()}
                                <button
                                    type="button"
                                    style="background: none; border: none; cursor: pointer; padding: 0; line-height: 1;"
                                    on:click=move |_| selected_tags.update(|v| v.retain(|x| x != &tag))
                                >"×"</button>
                            </span>
                        }
                    }).collect_view()}
                </div>
                <input
                    type="text"
                    class="form-input"
                    bind:value=tags
                    placeholder="tag1 tag2 tag3"
                    on:keydown=move |e| {
                        if e.key() == "Enter" {
                            e.prevent_default();
                            commit_tags();
                        }
                    }
                    on:blur=move |_| commit_tags()
                />
            </div>
        </Modal>
    }
}

fn odd(i: usize) -> &'static str {
    match i.is_multiple_of(2) {
        true => "link",
        false => "link odd",
    }
}
