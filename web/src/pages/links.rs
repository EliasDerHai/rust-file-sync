use crate::api;
use crate::components::{EmptyState, Loading, Message, TagBadge, ToastSignal};
use leptos::prelude::*;

#[component]
pub fn LinksPage() -> impl IntoView {
    let (trigger, set_trigger) = signal(0u32);
    let links = LocalResource::new(move || {
        trigger.get();
        api::fetch_links()
    });
    let msg = ToastSignal::new();
    let selected_tags: RwSignal<Vec<String>> = RwSignal::new(vec![]);

    let badge_click = move |t: String| {
        Callback::new(move |_| {
            selected_tags.update(|v| {
                if v.contains(&t) {
                    v.retain(|x| x != &t);
                } else {
                    v.push(t.clone());
                }
            });
        })
    };

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
                                                let t = tag.clone();
                                                let tag_for_active = tag.clone();
                                                view! {
                                                    <TagBadge
                                                        tag=tag
                                                        active=Signal::derive(move || selected_tags.get().contains(&tag_for_active))
                                                        on_click={badge_click(t)}
                                                    />
                                                }
                                            }).collect_view()}
                                        </div>
                                    })}

                                    <ul style="list-style: none; padding: 0;">
                                        {move || {
                                            let active = selected_tags.get();
                                            links.get_value().into_iter()
                                                .filter(|l| active.is_empty() || active.iter().all(|t| l.tags.contains(t)))
                                                .enumerate()
                                                .map(|(i, link)| view! {
                                                    <div class={odd(i)}>
                                                        <a href={link.url.clone()} target="_blank" style="padding-right: 1em">{link.link_text(30)}</a>
                                                        {link.tags.iter().map(|t| {
                                                            let t = t.clone();
                                                            let t_active = t.clone();
                                                            view! {
                                                                <TagBadge
                                                                    tag=t.clone()
                                                                    active=Signal::derive(move || selected_tags.get().contains(&t_active))
                                                                    on_click={badge_click(t)}
                                                                />
                                                            }
                                                        }).collect_view()}
                                                    </div>
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

fn odd(i: usize) -> &'static str {
    match i.is_multiple_of(2) {
        true => "link",
        false => "link odd",
    }
}
