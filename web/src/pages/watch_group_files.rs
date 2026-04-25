use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use shared::dtos::{is_image, FileDescription};
use std::collections::HashSet;

use crate::api;
use crate::components::{
    EmptyState, FileIcon, FileIconLarge, FolderIcon, FolderIconLarge, Loading, TextFileIconLarge,
};

#[derive(Clone, PartialEq)]
enum ViewMode {
    List,
    Tile,
}

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

fn is_text(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" | "sh" | "log"
    )
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
pub fn WatchGroupFilesPage() -> impl IntoView {
    let params = use_params_map();
    let wg_id: Option<i64> = params.with_untracked(|p| p.get("id").and_then(|s| s.parse().ok()));

    let Some(id) = wg_id else {
        return view! {
            <div class="container">
                <div class="message message-error">"Invalid watch group ID"</div>
            </div>
        }
        .into_any();
    };

    let files = LocalResource::new(move || api::fetch_watch_group_files(id));
    let current_path: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let view_mode: RwSignal<ViewMode> = RwSignal::new(ViewMode::List);

    view! {
        <div class="container">
            <A href="/app/watch-groups" attr:class="btn btn-secondary">"← Back"</A>
            <h1>"Watch Group Files"</h1>
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match files.await {
                        Ok(file_list) => {
                            if file_list.is_empty() {
                                view! {
                                    <EmptyState message="No files in this watch group yet." />
                                }
                                .into_any()
                            } else {
                                view! {
                                    <div class="filetree-toolbar">
                                        <Breadcrumb current_path />
                                        <div class="flex gap-1">
                                            <button
                                                class="btn btn-secondary"
                                                on:click=move |_| view_mode.set(ViewMode::List)
                                            >
                                                "List"
                                            </button>
                                            <button
                                                class="btn btn-secondary"
                                                on:click=move |_| view_mode.set(ViewMode::Tile)
                                            >
                                                "Tile"
                                            </button>
                                        </div>
                                    </div>
                                    <FiletreeView
                                        all_files=file_list
                                        current_path
                                        view_mode
                                        wg_id=id
                                    />
                                }
                                .into_any()
                            }
                        }
                        Err(e) => {
                            view! { <div class="message message-error">"Error: " {e}</div> }
                                .into_any()
                        }
                    }
                })}
            </Suspense>
        </div>
    }
    .into_any()
}

#[component]
fn Breadcrumb(current_path: RwSignal<Vec<String>>) -> impl IntoView {
    view! {
        <div class="breadcrumb">
            <button
                class="breadcrumb-item"
                on:click=move |_| current_path.set(vec![])
            >
                "Root"
            </button>
            {move || {
                let path = current_path.get();
                path.clone()
                    .into_iter()
                    .enumerate()
                    .map(|(i, seg)| {
                        let path_until = path[..=i].to_vec();
                        view! {
                            <span class="breadcrumb-sep">" › "</span>
                            <button
                                class="breadcrumb-item"
                                on:click=move |_| current_path.set(path_until.clone())
                            >
                                {seg}
                            </button>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

#[component]
fn FiletreeView(
    all_files: Vec<FileDescription>,
    current_path: RwSignal<Vec<String>>,
    view_mode: RwSignal<ViewMode>,
    wg_id: i64,
) -> impl IntoView {
    view! {
        {move || {
            let dir = current_path.get();
            let (dirs, files_here) = files_at_depth(&all_files, &dir);
            match view_mode.get() {
                ViewMode::List => {
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
                                    let href = if is_image(&file.file_type) {
                                        api::gallery_url(wg_id, &path_str)
                                    } else {
                                        api::watch_group_file_preview_url(wg_id, &path_str)
                                    };
                                    let file_name = file.file_name.clone();
                                    let size = format_size(file.size_in_bytes);
                                    view! {
                                        <li>
                                            <a
                                                class="filetree-row"
                                                href=href
                                                target="_blank"
                                            >
                                                <FileIcon />
                                                <span>{file_name}</span>
                                                <span class="filetree-row-meta">{size}</span>
                                            </a>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    }
                    .into_any()
                }
                ViewMode::Tile => {
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
                                    if is_image(&ext) {
                                        let gallery_href = api::gallery_url(wg_id, &path_str);
                                        view! {
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
                                        }
                                        .into_any()
                                    } else {
                                        let text_file = is_text(&ext);
                                        view! {
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
                                        }
                                        .into_any()
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                    .into_any()
                }
            }
        }}
    }
}
