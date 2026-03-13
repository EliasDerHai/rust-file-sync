use crate::components::{EmptyState, Loading, Message, ToastSignal};
use leptos::prelude::*;
use shared::dtos::LinkDto;

use crate::api;

#[component]
pub fn LinksPage() -> impl IntoView {
    let (trigger, set_trigger) = signal(0u32);
    let links = LocalResource::new(move || {
        trigger.get();
        api::fetch_links()
    });
    let msg = ToastSignal::new();

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
                                view! {
                                    <ul style="list-style: none; padding: 0;">
                                        {links.into_iter().enumerate().map(|(i, link)| {
                                            view! { <a class={odd(i)} href={link.url}>{link.link_text(30)}</a>}
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

fn odd(i: usize) -> &'static str {
    match i.is_multiple_of(2) {
        true => "link",
        false => "link odd",
    }
}
