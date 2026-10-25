use leptos::prelude::*;
use shared::dtos::FileDescription;
use shared::media::MediaKind;
use std::cmp::Reverse;
use std::collections::HashSet;

use crate::api;
use crate::components::{
    AudioIconLarge, FileIcon, FileIconLarge, FolderIcon, FolderIconLarge, TextFileIconLarge,
    VideoIconLarge,
};
use crate::format::format_size;
use crate::pages::watch_group_files::{SortMode, ViewMode};

fn files_at_depth(all: &[FileDescription], dir: &[String]) -> (Vec<String>, Vec<FileDescription>) {
    let depth = dir.len();
    let files: Vec<FileDescription> = all
        .iter()
        .filter(|f| {
            let segs = f.relative_path.get();
            segs.len() == depth + 1 && segs.starts_with(dir)
        })
        .cloned()
        .collect();
    let mut dirs: Vec<String> = all
        .iter()
        .filter(|f| {
            let segs = f.relative_path.get();
            segs.len() > depth + 1 && segs.starts_with(dir)
        })
        .map(|f| f.relative_path.get()[depth].clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    dirs.sort();
    (dirs, files)
}

#[component]
pub fn FiletreeView(
    all_files: RwSignal<Vec<FileDescription>>,
    current_path: RwSignal<Vec<String>>,
    view_mode: RwSignal<ViewMode>,
    sort_mode: RwSignal<SortMode>,
    wg_id: i64,
    selected: RwSignal<HashSet<String>>,
    gallery: RwSignal<Option<String>>,
) -> impl IntoView {
    move || {
        let dir = current_path.get();
        let (dirs, mut files_here) = all_files.with(|all| files_at_depth(all, &dir));
        match sort_mode.get() {
            SortMode::Alphabetical => files_here.sort_by_key(|desc| desc.file_name.clone()),
            SortMode::Latest => {
                files_here.sort_by_key(|desc| Reverse(desc.last_updated_utc_millis))
            }
        }

        match view_mode.get() {
            ViewMode::List => {
                file_tree_list_view(dirs, files_here, current_path, wg_id, selected, gallery)
                    .into_any()
            }
            ViewMode::Tile => {
                file_tree_tile_view(dirs, files_here, current_path, wg_id, selected, gallery)
                    .into_any()
            }
        }
    }
}

/// Media opens the gallery overlay; everything else opens the raw file in a new tab. The href
/// is the raw file either way, so a modified click (new tab, long-press) still gets the file.
#[component]
fn FileLink(
    wg_id: i64,
    path: String,
    is_media: bool,
    gallery: RwSignal<Option<String>>,
    class: &'static str,
    children: Children,
) -> impl IntoView {
    let href = api::watch_group_file_preview_url(wg_id, &path);
    if is_media {
        let on_click = move |ev: web_sys::MouseEvent| {
            if ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() {
                return;
            }
            ev.prevent_default();
            gallery.set(Some(path.clone()));
        };
        view! { <a class=class href=href on:click=on_click>{children()}</a> }.into_any()
    } else {
        view! { <a class=class href=href target="_blank">{children()}</a> }.into_any()
    }
}

fn file_tree_list_view(
    dirs: Vec<String>,
    files_here: Vec<FileDescription>,
    current_path: RwSignal<Vec<String>>,
    wg_id: i64,
    selected: RwSignal<HashSet<String>>,
    gallery: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <ul class="filetree-list">
            {dirs
                .into_iter()
                .map(|name| {
                    let click_name = name.clone();
                    view! {
                        <li>
                            <div
                                class="filetree-row"
                                on:click=move |_| {
                                    current_path.update(|p| p.push(click_name.clone()))
                                }
                            >
                                <FolderIcon />
                                <span>{name}</span>
                            </div>
                        </li>
                    }
                })
                .collect_view()}
            {files_here
                .into_iter()
                .map(|file| {
                    let path_str = file.relative_path.to_serialized_string();
                    let is_media = MediaKind::from(&file.file_type).is_media();
                    let file_name = file.file_name.clone();
                    let size = format_size(file.size_in_bytes);
                    let p_link = path_str.clone();
                    let p_class = path_str.clone();
                    let p_check = path_str.clone();
                    let p_toggle = path_str;
                    let is_selected_class = move || selected.get().contains(&p_class);
                    let is_selected_check = move || selected.get().contains(&p_check);
                    let on_toggle = move |_| {
                        selected.update(|s| {
                            if s.contains(&p_toggle) {
                                s.remove(&p_toggle);
                            } else {
                                s.insert(p_toggle.clone());
                            }
                        });
                    };
                    view! {
                        <li>
                            <div
                                class="filetree-row-selectable"
                                class:filetree-row-selected=is_selected_class
                            >
                                <input
                                    type="checkbox"
                                    class="filetree-checkbox"
                                    prop:checked=is_selected_check
                                    on:change=on_toggle
                                />
                                <FileLink
                                    wg_id
                                    path=p_link
                                    is_media
                                    gallery
                                    class="filetree-row"
                                >
                                    <FileIcon />
                                    <span>{file_name}</span>
                                    <span class="filetree-row-meta">{size}</span>
                                </FileLink>
                            </div>
                        </li>
                    }
                })
                .collect_view()}
        </ul>
    }
}

fn file_tree_tile_view(
    dirs: Vec<String>,
    files_here: Vec<FileDescription>,
    current_path: RwSignal<Vec<String>>,
    wg_id: i64,
    selected: RwSignal<HashSet<String>>,
    gallery: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="filetree-tile-grid">
            {dirs
                .into_iter()
                .map(|name| {
                    let click_name = name.clone();
                    view! {
                        <div
                            class="filetree-tile"
                            on:click=move |_| {
                                current_path.update(|p| p.push(click_name.clone()))
                            }
                        >
                            <div class="filetree-tile-preview">
                                <FolderIconLarge />
                            </div>
                            <span class="filetree-tile-name">{name}</span>
                        </div>
                    }
                })
                .collect_view()}
            {files_here
                .into_iter()
                .map(|file| {
                    let path_str = file.relative_path.to_serialized_string();
                    let kind = MediaKind::from(&file.file_type);
                    let is_media = kind.is_media();
                    // tiles only ever load small server thumbnails - never originals and never
                    // <video>/<audio>, which pulled data for every file and stalled the app.
                    // a 404 (heic, undecodable, no ffmpeg on the server) falls back to the icon
                    let is_video = kind == MediaKind::Video;
                    let preview = match kind {
                        MediaKind::Image | MediaKind::Video => {
                            let thumb_url = api::watch_group_file_thumbnail_url(
                                wg_id,
                                &path_str,
                                file.last_updated_utc_millis.as_u64(),
                            );
                            let failed = RwSignal::new(false);
                            view! {
                                <Show
                                    when=move || !failed.get()
                                    fallback=move || {
                                        if is_video {
                                            view! { <VideoIconLarge /> }.into_any()
                                        } else {
                                            view! { <FileIconLarge /> }.into_any()
                                        }
                                    }
                                >
                                    <img
                                        src=thumb_url.clone()
                                        class="filetree-tile-img"
                                        loading="lazy"
                                        decoding="async"
                                        on:error=move |_| failed.set(true)
                                    />
                                </Show>
                            }
                            .into_any()
                        }
                        MediaKind::Audio => view! { <AudioIconLarge /> }.into_any(),
                        MediaKind::Text => view! { <TextFileIconLarge /> }.into_any(),
                        _ => view! { <FileIconLarge /> }.into_any(),
                    };
                    let file_name = file.file_name.clone();
                    let p_link = path_str.clone();
                    let p_class = path_str.clone();
                    let p_check = path_str.clone();
                    let p_toggle = path_str;
                    let is_selected_class = move || selected.get().contains(&p_class);
                    let is_selected_check = move || selected.get().contains(&p_check);
                    let on_toggle = move |_| {
                        selected.update(|s| {
                            if s.contains(&p_toggle) {
                                s.remove(&p_toggle);
                            } else {
                                s.insert(p_toggle.clone());
                            }
                        });
                    };
                    view! {
                        <div
                            class="filetree-tile-wrapper"
                            class:filetree-tile-selected=is_selected_class
                        >
                            <input
                                type="checkbox"
                                class="filetree-tile-checkbox"
                                prop:checked=is_selected_check
                                on:change=on_toggle
                            />
                            <FileLink wg_id path=p_link is_media gallery class="filetree-tile">
                                <div
                                    class="filetree-tile-preview"
                                    class:filetree-tile-preview-video=is_video
                                >
                                    {preview}
                                </div>
                                <span class="filetree-tile-name">{file_name}</span>
                            </FileLink>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}
