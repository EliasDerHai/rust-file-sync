use crate::api;
use crate::components::Loading;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_query_map;
use shared::dtos::LinkCreateDto;

/// Target of the manifest's `share_target`: the Android share sheet opens
/// `/app/share?title=&text=&url=`, and the link is saved right away.
#[component]
pub fn ShareLinkPage() -> impl IntoView {
    let params = use_query_map().get_untracked();
    let get = |key: &str| params.get(key).filter(|v| !v.trim().is_empty());

    // Android browsers usually put the URL in `text` rather than `url`.
    let url = get("url")
        .or_else(|| get("text").and_then(|t| find_url(&t)))
        .or_else(|| get("title").and_then(|t| find_url(&t)));
    let title = get("title").filter(|t| find_url(t).is_none());

    let saved = LocalResource::new(move || {
        let url = url.clone();
        let title = title.clone();
        async move {
            let url = url.ok_or_else(|| "No link found in the shared content.".to_string())?;
            api::create_link(LinkCreateDto {
                url: url.trim().to_string(),
                title,
            })
            .await
            .map(|()| url)
        }
    });

    view! {
        <div class="container">
            <h1>"Share link"</h1>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match saved.await {
                        Ok(url) => view! {
                            <div class="message message-success">"Saved " {url}</div>
                            <A href="/app/links">"Open links"</A>
                        }.into_any(),
                        Err(e) => view! { <div class="message message-error">"Error: " {e}</div> }.into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}

fn find_url(s: &str) -> Option<String> {
    s.split_whitespace()
        .find(|w| w.starts_with("http://") || w.starts_with("https://"))
        .map(str::to_string)
}
