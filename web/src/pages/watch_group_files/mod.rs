use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use std::collections::HashSet;

use crate::api;
use crate::components::{
    EmptyState, HomeIcon, Loading, Message, ToastSignal, TrashIcon, UploadIcon,
};

mod file_tree_view;
mod gallery;
use file_tree_view::FiletreeView;
use gallery::GalleryViewer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Tile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Latest,
    Alphabetical,
}

impl std::fmt::Display for SortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SortMode::Alphabetical => "alphabetical",
            SortMode::Latest => "latest",
        })
    }
}

impl From<&str> for SortMode {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "alphabetical" => SortMode::Alphabetical,
            "latest" => SortMode::Latest,
            _ => SortMode::Latest,
        }
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

    let (refresh_trigger, set_refresh_trigger) = signal(0u32);
    let selected: RwSignal<HashSet<String>> = RwSignal::new(HashSet::new());
    let msg = ToastSignal::new();
    let current_path: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let view_mode: RwSignal<ViewMode> = RwSignal::new(ViewMode::List);
    let sort_mode: RwSignal<SortMode> = RwSignal::new(SortMode::Latest);
    // path of the file the gallery overlay was opened on; the viewer tracks the current file itself
    let gallery: RwSignal<Option<String>> = RwSignal::new(None);
    let gallery_open = Memo::new(move |_| gallery.with(Option::is_some));

    // Clear selection whenever the user navigates into a different directory
    Effect::new(move |_| {
        let _ = current_path.get();
        selected.update(|s| s.clear());
    });

    let files = LocalResource::new(move || {
        refresh_trigger.get();
        api::fetch_watch_group_files(id)
    });

    let on_delete_click = move |_| {
        let paths: Vec<String> = selected.get_untracked().into_iter().collect();
        if paths.is_empty() {
            return;
        }
        let ok = web_sys::window()
            .unwrap()
            .confirm_with_message(&format!(
                "Delete {} file(s)? This cannot be undone.",
                paths.len()
            ))
            .unwrap_or(false);
        if !ok {
            return;
        }
        spawn_local(async move {
            for path in &paths {
                if let Err(e) = api::delete_watch_group_file(id, path).await {
                    msg.error(format!("Delete failed: {e}"));
                    return;
                }
            }
            let count = paths.len();
            selected.update(|s| s.clear());
            set_refresh_trigger.update(|t| *t += 1);
            msg.success(format!("Deleted {count} file(s)."));
        });
    };

    view! {
        <div class="container">
            <h1>"Watch Group Files"</h1>
            <Message signal=msg />
            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match files.await {
                        Ok(file_list) => {
                            if file_list.is_empty() {
                                view! {
                                    <div class="filetree-toolbar">
                                        <Breadcrumb current_path />
                                        <UploadButton
                                            wg_id=id
                                            current_path
                                            existing=vec![]
                                            msg
                                            set_refresh_trigger
                                        />
                                    </div>
                                    <EmptyState message="No files in this watch group yet." />
                                }
                                .into_any()
                            } else {
                                let existing = file_list
                                    .iter()
                                    .map(|f| f.relative_path.get().clone())
                                    .collect();
                                // shared with the gallery overlay so deleting there updates the list
                                let all_files = RwSignal::new(file_list);
                                view! {
                                    <div class="filetree-toolbar">
                                        <Breadcrumb current_path />
                                        <div class="flex gap-1">
                                            <Show when=move || !selected.get().is_empty()>
                                                <button
                                                    class="btn btn-danger"
                                                    on:click=on_delete_click
                                                >
                                                    <TrashIcon />
                                                    " Delete ("
                                                    {move || selected.get().len()}
                                                    ")"
                                                </button>
                                            </Show>
                                            <UploadButton
                                                wg_id=id
                                                current_path
                                                existing
                                                msg
                                                set_refresh_trigger
                                            />
                                            <select
                                                class="btn btn-secondary"
                                                prop:value=move || sort_mode.get().to_string()
                                                on:change=move |v| {
                                                    sort_mode.set(event_target_value(&v).as_str().into())
                                                }
                                            >
                                                <option value="latest">"Latest"</option>
                                                <option value="alphabetical">"Alphabetical"</option>
                                            </select>
                                            <select
                                                class="btn btn-secondary"
                                                prop:value=move || match view_mode.get() {
                                                    ViewMode::List => "list",
                                                    ViewMode::Tile => "tile",
                                                }
                                                on:change=move |v| {
                                                    view_mode.set(match event_target_value(&v).as_str() {
                                                        "tile" => ViewMode::Tile,
                                                        _ => ViewMode::List,
                                                    })
                                                }
                                            >
                                                <option value="list">"List"</option>
                                                <option value="tile">"Tile"</option>
                                            </select>
                                        </div>
                                    </div>
                                    <FiletreeView
                                        all_files
                                        current_path
                                        view_mode
                                        sort_mode
                                        wg_id=id
                                        selected
                                        gallery
                                    />
                                    <Show when=move || gallery_open.get()>
                                        <GalleryViewer
                                            all_files
                                            initial_path=gallery.get_untracked().unwrap_or_default()
                                            wg_id=id
                                            sort_mode=sort_mode.get_untracked()
                                            on_close=Callback::new(move |_| gallery.set(None))
                                        />
                                    </Show>
                                }
                                .into_any()
                            }
                        }
                        Err(e) => {
                            view! {
                                <div class="filetree-toolbar">
                                    <Breadcrumb current_path />
                                </div>
                                <div class="message message-error">"Error: " {e}</div>
                            }
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
        <div class="flex-center gap-1">
            <A href="/app/watch-groups" attr:class="btn btn-secondary">"← Back"</A>
            <button
                class="btn btn-secondary gap-1"
                on:click=move |_| current_path.set(vec![])
            >
                <HomeIcon />
                "Root"
            </button>
            <div class="breadcrumb">
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
        </div>
    }
}

/// Hidden multi-file input behind a button; uploads into the directory currently shown.
#[component]
fn UploadButton(
    wg_id: i64,
    current_path: RwSignal<Vec<String>>,
    /// relative paths already in the watch group, to confirm before overwriting
    existing: Vec<Vec<String>>,
    msg: ToastSignal,
    set_refresh_trigger: WriteSignal<u32>,
) -> impl IntoView {
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let uploading = RwSignal::new(false);

    let on_change = move |_| {
        let Some(input) = input_ref.get_untracked() else {
            return;
        };
        let files: Vec<web_sys::File> = input
            .files()
            .map(|list| (0..list.length()).filter_map(|i| list.get(i)).collect())
            .unwrap_or_default();
        // reset so picking the same file again still fires `change`
        input.set_value("");
        if files.is_empty() {
            return;
        }

        let dir = current_path.get_untracked();
        let clashes = files
            .iter()
            .filter(|f| {
                let mut path = dir.clone();
                path.push(f.name());
                existing.contains(&path)
            })
            .count();
        if clashes > 0 {
            let ok = web_sys::window()
                .unwrap()
                .confirm_with_message(&format!("Overwrite {clashes} existing file(s)?"))
                .unwrap_or(false);
            if !ok {
                return;
            }
        }

        uploading.set(true);
        let dir = dir.join("/");
        spawn_local(async move {
            for file in &files {
                if let Err(e) = api::upload_watch_group_file(wg_id, &dir, file).await {
                    uploading.set(false);
                    // earlier files of this batch did land
                    set_refresh_trigger.update(|t| *t += 1);
                    msg.error(format!("Upload of '{}' failed: {e}", file.name()));
                    return;
                }
            }
            uploading.set(false);
            set_refresh_trigger.update(|t| *t += 1);
            msg.success(format!("Uploaded {} file(s).", files.len()));
        });
    };

    view! {
        <input type="file" multiple style="display: none" node_ref=input_ref on:change=on_change />
        <button
            class="btn btn-secondary gap-1"
            disabled=move || uploading.get()
            on:click=move |_| {
                if let Some(input) = input_ref.get_untracked() {
                    input.click();
                }
            }
        >
            <UploadIcon />
            {move || if uploading.get() { "Uploading…" } else { "Upload" }}
        </button>
    }
}
