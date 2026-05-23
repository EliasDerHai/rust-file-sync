use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use crate::api;
use crate::components::{EmptyState, Loading, Message, ToastSignal, TrashIcon};

mod file_tree_view;
use file_tree_view::FiletreeView;

#[derive(Clone, PartialEq)]
pub enum ViewMode {
    List,
    Tile,
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
            <A href="/app/watch-groups" attr:class="btn btn-secondary">"← Back"</A>
            <h1>"Watch Group Files"</h1>
            <Message signal=msg />
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
                                        selected
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
