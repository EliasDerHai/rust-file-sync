use std::cmp::Reverse;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use shared::dtos::FileDescription;
use shared::media::MediaKind;
use wasm_bindgen::JsValue;

use crate::api;
use crate::components::{CloseIcon, Loading, Message, ToastSignal, TrashIcon};
use crate::pages::SortMode;

/// clips shorter than this loop, so short videos behave like the gifs they replace
const LOOP_THRESHOLD_SECONDS: f64 = 8.0;

/// a detached media element keeps downloading until it's garbage collected -
/// dropping `src` and calling `load()` is what actually aborts the request
fn release_media(el: &web_sys::HtmlMediaElement) {
    let _ = el.pause();
    let _ = el.remove_attribute("src");
    el.load();
}

fn media_in_same_dir(
    all: &[FileDescription],
    current_path: &str,
    sort_mode: &SortMode,
) -> Vec<FileDescription> {
    let current_segments: Vec<&str> = current_path.split('/').collect();
    let dir_segments = &current_segments[..current_segments.len().saturating_sub(1)];

    let mut media: Vec<FileDescription> = all
        .iter()
        .filter(|f| {
            if !MediaKind::from(&f.file_type).is_media() {
                return false;
            }
            let segs = f.relative_path.get();
            if segs.len() != dir_segments.len() + 1 {
                return false;
            }
            segs[..dir_segments.len()]
                .iter()
                .zip(dir_segments.iter())
                .all(|(a, b)| a == b)
        })
        .cloned()
        .collect();

    match sort_mode {
        SortMode::Alphabetical => media.sort_by_key(|desc| desc.file_name.clone()),
        SortMode::Latest => media.sort_by_key(|desc| Reverse(desc.last_updated_utc_millis)),
    }

    media
}

#[component]
pub fn MediaGalleryPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();

    let wg_id: Option<i64> = params.with_untracked(|p| p.get("id").and_then(|s| s.parse().ok()));
    let path: Option<String> = query.with_untracked(|q| q.get("path").filter(|s| !s.is_empty()));
    let sort_mode: SortMode = query
        .with_untracked(|q| q.get("sort").filter(|s| !s.is_empty()))
        .unwrap_or_default()
        .as_str()
        .into();

    let (Some(id), Some(current_path)) = (wg_id, path) else {
        return view! {
            <div class="container">
                <div class="message message-error">"Invalid gallery URL"</div>
            </div>
        }
        .into_any();
    };

    let files = LocalResource::new(move || api::fetch_watch_group_files(id));
    let current_path_signal = RwSignal::new(current_path);

    view! {
        <Suspense fallback=Loading>
            {move || Suspend::new(async move {
                match files.await {
                    Ok(file_list) => {
                        let all_files = RwSignal::new(file_list);
                        view! {
                            <GalleryViewer
                                all_files=all_files
                                current_path=current_path_signal
                                wg_id=id
                                sort_mode=sort_mode
                            />
                        }
                        .into_any()
                    }
                    Err(e) => {
                        view! {
                            <div class="container">
                                <div class="message message-error">"Error: " {e}</div>
                            </div>
                        }
                        .into_any()
                    }
                }
            })}
        </Suspense>
    }
    .into_any()
}

fn path_of(f: &FileDescription) -> String {
    f.relative_path.to_serialized_string()
}

fn video_view(src: String) -> impl IntoView {
    let video_ref = NodeRef::<leptos::html::Video>::new();
    // runs when the slide stops being the active one and when leaving the gallery
    on_cleanup(move || {
        if let Some(el) = video_ref.get_untracked() {
            release_media(&el);
        }
    });
    view! {
        <video
            node_ref=video_ref
            class="gallery-video"
            src=src
            controls
            autoplay
            muted
            playsinline
            on:loadedmetadata=|ev| {
                let el = event_target::<web_sys::HtmlVideoElement>(&ev);
                let duration = el.duration();
                if duration.is_finite() && duration < LOOP_THRESHOLD_SECONDS {
                    el.set_loop(true);
                }
            }
        />
    }
}

fn audio_view(src: String) -> impl IntoView {
    let audio_ref = NodeRef::<leptos::html::Audio>::new();
    on_cleanup(move || {
        if let Some(el) = audio_ref.get_untracked() {
            release_media(&el);
        }
    });
    view! { <audio node_ref=audio_ref class="gallery-audio" src=src controls autoplay /> }
}

/// One swipe page. Every media file of the directory gets a page so the scroll width
/// matches the file count, but only the current file and its neighbours (preloaded so a
/// swipe never shows a blank) hold real content; video and audio only while current.
#[component]
fn GallerySlide(
    index: ReadSignal<usize>,
    current: Memo<usize>,
    wg_id: i64,
    file: FileDescription,
) -> impl IntoView {
    let near = Memo::new(move |_| current.get().abs_diff(index.get()) <= 1);
    let active = Memo::new(move |_| current.get() == index.get());
    let src = api::watch_group_file_preview_url(wg_id, &path_of(&file));
    let kind = MediaKind::from(&file.file_type);

    view! {
        <div class="gallery-slide">
            {move || match kind {
                MediaKind::Video => active.get().then(|| video_view(src.clone()).into_any()),
                MediaKind::Audio => active.get().then(|| audio_view(src.clone()).into_any()),
                _ => near.get().then(|| view! { <img class="gallery-img" src=src.clone() /> }.into_any()),
            }}
        </div>
    }
}

#[component]
fn GalleryViewer(
    all_files: RwSignal<Vec<FileDescription>>,
    current_path: RwSignal<String>,
    wg_id: i64,
    sort_mode: SortMode,
) -> impl IntoView {
    let msg = ToastSignal::new();
    let navigate = use_navigate();
    let navigate_sv = StoredValue::new(navigate);
    let track_ref = NodeRef::<leptos::html::Div>::new();

    // the directory is fixed for the lifetime of the gallery, only its files change (delete)
    let dir_path = current_path.get_untracked();
    let media = Memo::new(move |_| media_in_same_dir(&all_files.get(), &dir_path, &sort_mode));
    let idx = Memo::new(move |_| {
        let path = current_path.get();
        media
            .with(|m| m.iter().position(|f| path_of(f) == path))
            .unwrap_or(0)
    });

    if !media.with_untracked(|m| m.iter().any(|f| path_of(f) == current_path.get_untracked())) {
        return view! {
            <div class="container">
                <div class="message message-error">"File not found"</div>
            </div>
        }
        .into_any();
    }

    let scroll_to_index = move |i: usize, smooth: bool| {
        if let Some(track) = track_ref.get_untracked() {
            let opts = web_sys::ScrollToOptions::new();
            opts.set_left(i as f64 * track.client_width() as f64);
            opts.set_behavior(if smooth {
                web_sys::ScrollBehavior::Smooth
            } else {
                web_sys::ScrollBehavior::Instant
            });
            track.scroll_to_with_scroll_to_options(&opts);
        }
    };
    let go_to = move |i: usize| {
        if i < media.with_untracked(|m| m.len()) {
            scroll_to_index(i, true);
        }
    };

    // the track follows the finger on its own; once it has come to rest on a page, adopt
    // that page as the current file
    let on_scroll_end = move |_| {
        let Some(track) = track_ref.get_untracked() else {
            return;
        };
        let width = track.client_width() as f64;
        if width <= 0.0 {
            return;
        }
        let i = (track.scroll_left() as f64 / width).round() as usize;
        if i != idx.get_untracked()
            && let Some(path) = media.with_untracked(|m| m.get(i).map(path_of))
        {
            current_path.set(path);
        }
    };

    // current file changed without a swipe (initial file, delete): move the track to it
    Effect::new(move |_| {
        let i = idx.get();
        if let Some(track) = track_ref.get() {
            let target = i as f64 * track.client_width() as f64;
            if (track.scroll_left() as f64 - target).abs() > 2.0 {
                scroll_to_index(i, false);
            }
        }
    });

    // keep the address bar on the current file (reload stays here, Back leaves the gallery)
    Effect::new(move |_| {
        let url = api::gallery_url(wg_id, &current_path.get(), sort_mode);
        if let Some(history) = web_sys::window().and_then(|w| w.history().ok()) {
            let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&url));
        }
    });

    window_event_listener(leptos::ev::keydown, move |ev| match ev.key().as_str() {
        "ArrowLeft" => go_to(idx.get_untracked().saturating_sub(1)),
        "ArrowRight" => go_to(idx.get_untracked() + 1),
        _ => {}
    });

    let on_close = move |_| {
        let history = web_sys::window().and_then(|w| w.history().ok());
        match history {
            Some(h) if h.length().unwrap_or(1) > 1 => {
                let _ = h.back();
            }
            _ => navigate_sv.get_value()(&format!("/app/watch-groups/{wg_id}"), Default::default()),
        }
    };

    let on_delete_click = move |_| {
        let i = idx.get_untracked();
        let Some((path, file_name, neighbour)) = media.with_untracked(|m| {
            let f = m.get(i)?;
            let neighbour = m.get(i + 1).or(i.checked_sub(1).and_then(|p| m.get(p)));
            Some((path_of(f), f.file_name.clone(), neighbour.map(path_of)))
        }) else {
            return;
        };
        let ok = web_sys::window()
            .unwrap()
            .confirm_with_message(&format!("Delete '{file_name}'? This cannot be undone."))
            .unwrap_or(false);
        if !ok {
            return;
        }
        spawn_local(async move {
            match api::delete_watch_group_file(wg_id, &path).await {
                Ok(()) => match neighbour {
                    Some(n) => batch(move || {
                        all_files.update(|v| v.retain(|f| path_of(f) != path));
                        current_path.set(n);
                    }),
                    None => navigate_sv.get_value()(
                        &format!("/app/watch-groups/{wg_id}"),
                        Default::default(),
                    ),
                },
                Err(e) => msg.error(e),
            }
        });
    };

    // computed here: a `>` inside a view! attribute is parsed as the end of the tag
    let at_first = move || idx.get() == 0;
    let at_last = move || idx.get() + 1 >= media.with(|m| m.len());

    view! {
        <div class="gallery-container">
            <div class="gallery-top-bar">
                <button class="btn btn-icon btn-secondary" title="Close" on:click=on_close>
                    <CloseIcon />
                </button>
                <Message signal=msg />
                <button
                    class="btn btn-icon btn-danger gallery-delete-btn"
                    title="Delete file"
                    on:click=on_delete_click
                >
                    <TrashIcon />
                </button>
            </div>
            <div class="gallery-track" node_ref=track_ref on:scrollend=on_scroll_end>
                <ForEnumerate
                    each=move || media.get()
                    key=path_of
                    children=move |index, file| {
                        view! { <GallerySlide index current=idx wg_id file /> }
                    }
                />
            </div>
            <button
                class="gallery-nav-btn prev"
                disabled=at_first
                on:click=move |_| go_to(idx.get_untracked().saturating_sub(1))
            >
                "\u{2039}"
            </button>
            <button
                class="gallery-nav-btn next"
                disabled=at_last
                on:click=move |_| go_to(idx.get_untracked() + 1)
            >
                "\u{203A}"
            </button>
            <div class="gallery-filename">
                {move || media.with(|m| m.get(idx.get()).map(|f| f.file_name.clone()))}
            </div>
        </div>
    }
    .into_any()
}
