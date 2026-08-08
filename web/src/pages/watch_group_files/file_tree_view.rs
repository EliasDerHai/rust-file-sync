use leptos::prelude::*;
use shared::dtos::FileDescription;
use shared::media::MediaKind;
use std::collections::HashSet;

use crate::api;
use crate::components::{FileIcon, FileIconLarge, FolderIcon, FolderIconLarge, TextFileIconLarge};
use crate::pages::watch_group_files::ViewMode;

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

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[component]
pub fn FiletreeView(
    all_files: Vec<FileDescription>,
    current_path: RwSignal<Vec<String>>,
    view_mode: RwSignal<ViewMode>,
    wg_id: i64,
    selected: RwSignal<HashSet<String>>,
) -> impl IntoView {
    move || {
        let dir = current_path.get();
        let (dirs, mut files_here) = files_at_depth(&all_files, &dir);
        files_here.sort_by_key(|desc| desc.file_name.clone());

        match view_mode.get() {
            ViewMode::List => {
                file_tree_list_view(dirs, files_here, current_path, wg_id, selected).into_any()
            }
            ViewMode::Tile => {
                file_tree_tile_view(dirs, files_here, current_path, wg_id, selected).into_any()
            }
        }
    }
}

fn file_tree_list_view(
    dirs: Vec<String>,
    files_here: Vec<FileDescription>,
    current_path: RwSignal<Vec<String>>,
    wg_id: i64,
    selected: RwSignal<HashSet<String>>,
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
                    let href = if MediaKind::from(&file.file_type).is_media() {
                        api::gallery_url(wg_id, &path_str)
                    } else {
                        api::watch_group_file_preview_url(wg_id, &path_str)
                    };
                    let file_name = file.file_name.clone();
                    let size = format_size(file.size_in_bytes);
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
                                <a
                                    class="filetree-row"
                                    href=href
                                    target="_blank"
                                >
                                    <FileIcon />
                                    <span>{file_name}</span>
                                    <span class="filetree-row-meta">{size}</span>
                                </a>
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
                            <FolderIconLarge />
                            <span class="filetree-tile-name">{name}</span>
                        </div>
                    }
                })
                .collect_view()}
            {files_here
                .into_iter()
                .map(|file| {
                    let path_str = file.relative_path.to_serialized_string();
                    let raw_url = api::watch_group_file_preview_url(
                        wg_id,
                        &path_str,
                    );
                    let file_name = file.file_name.clone();
                    let ext = file.file_type.clone();
                    let p_class = path_str.clone();
                    let p_check = path_str.clone();
                    let p_toggle = path_str.clone();
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
                    match MediaKind::from(&ext) {
                        MediaKind::Image => {
                            let gallery_href = api::gallery_url(wg_id, &path_str);
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
                                    <a
                                        class="filetree-tile"
                                        href=gallery_href
                                        target="_blank"
                                    >
                                        <img
                                            src=raw_url
                                            class="filetree-tile-img"
                                            loading="lazy"
                                        />
                                        <span class="filetree-tile-name">{file_name}</span>
                                    </a>
                                </div>
                            }
                            .into_any()
                        }
                        // playable in place, so the tile itself must not be a link —
                        // clicking the controls would navigate away. only the name links.
                        MediaKind::Video => {
                            let gallery_href = api::gallery_url(wg_id, &path_str);
                            // the media fragment makes the browser render a real first
                            // frame instead of a black box (needs range requests)
                            let poster_url = format!("{raw_url}#t=0.1");
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
                                    <div class="filetree-tile filetree-tile-media-card">
                                        <video
                                            class="filetree-tile-media"
                                            src=poster_url
                                            controls
                                            preload="metadata"
                                            playsinline
                                        />
                                        <a
                                            class="filetree-tile-name"
                                            href=gallery_href
                                            target="_blank"
                                        >
                                            {file_name}
                                        </a>
                                    </div>
                                </div>
                            }
                            .into_any()
                        }
                        MediaKind::Audio => {
                            let gallery_href = api::gallery_url(wg_id, &path_str);
                            view! {
                                <div
                                    class="filetree-tile-wrapper filetree-tile-wrapper-audio"
                                    class:filetree-tile-selected=is_selected_class
                                >
                                    <input
                                        type="checkbox"
                                        class="filetree-tile-checkbox"
                                        prop:checked=is_selected_check
                                        on:change=on_toggle
                                    />
                                    <div class="filetree-tile filetree-tile-media-card">
                                        <audio
                                            class="filetree-tile-media filetree-tile-audio"
                                            src=raw_url
                                            controls
                                            preload="metadata"
                                        />
                                        <a
                                            class="filetree-tile-name"
                                            href=gallery_href
                                            target="_blank"
                                        >
                                            {file_name}
                                        </a>
                                    </div>
                                </div>
                            }
                            .into_any()
                        }
                        kind => {
                            let text_file = kind == MediaKind::Text;
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
                                    <a
                                        class="filetree-tile"
                                        href=raw_url
                                        target="_blank"
                                    >
                                        <Show
                                            when=move || text_file
                                            fallback=|| view! { <FileIconLarge /> }
                                        >
                                            <TextFileIconLarge />
                                        </Show>
                                        <span class="filetree-tile-name">{file_name}</span>
                                    </a>
                                </div>
                            }
                            .into_any()
                        }
                    }
                })
                .collect_view()}
        </div>
    }
}
