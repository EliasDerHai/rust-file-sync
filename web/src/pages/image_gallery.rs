use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use shared::dtos::{is_image, FileDescription};

use crate::api;
use crate::components::{Loading, Message, ToastSignal, TrashIcon};

fn images_in_same_dir(all: &[FileDescription], current_path: &str) -> Vec<FileDescription> {
    let current_segments: Vec<&str> = current_path.split('/').collect();
    let dir_segments = &current_segments[..current_segments.len().saturating_sub(1)];

    let mut images: Vec<FileDescription> = all
        .iter()
        .filter(|f| {
            if !is_image(&f.file_type) {
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

    images.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    images
}

#[component]
pub fn ImageGalleryPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();

    let wg_id: Option<i64> = params.with_untracked(|p| p.get("id").and_then(|s| s.parse().ok()));
    let path: Option<String> = query.with_untracked(|q| q.get("path").filter(|s| !s.is_empty()));

    let (Some(id), Some(current_path)) = (wg_id, path) else {
        return view! {
            <div class="gallery-container">
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
                            />
                        }
                        .into_any()
                    }
                    Err(e) => {
                        view! {
                            <div class="gallery-container">
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

#[component]
fn GalleryViewer(
    all_files: RwSignal<Vec<FileDescription>>,
    current_path: RwSignal<String>,
    wg_id: i64,
) -> impl IntoView {
    let msg = ToastSignal::new();
    let navigate = use_navigate();
    let navigate_sv = StoredValue::new(navigate);

    view! {
        {move || {
            let path = current_path.get();
            let images = images_in_same_dir(&all_files.get(), &path);
            let current_idx = images
                .iter()
                .position(|f| f.relative_path.to_serialized_string() == path);

            let Some(idx) = current_idx else {
                return view! {
                    <div class="gallery-container">
                        <div class="message message-error">"Image not found"</div>
                    </div>
                }
                .into_any();
            };

            let is_first = idx == 0;
            let is_last = idx == images.len() - 1;
            let img_src = api::watch_group_file_preview_url(
                wg_id,
                &images[idx].relative_path.to_serialized_string(),
            );
            let file_name = images[idx].file_name.clone();

            let prev_path = if !is_first {
                Some(images[idx - 1].relative_path.to_serialized_string())
            } else {
                None
            };
            let next_path = if !is_last {
                Some(images[idx + 1].relative_path.to_serialized_string())
            } else {
                None
            };

            let path_for_delete = path.clone();
            let file_name_for_delete = file_name.clone();
            let next_for_delete = next_path.clone();
            let prev_for_delete = prev_path.clone();
            let on_delete_click = move |_| {
                let ok = web_sys::window()
                    .unwrap()
                    .confirm_with_message(&format!(
                        "Delete '{file_name_for_delete}'? This cannot be undone."
                    ))
                    .unwrap_or(false);
                if !ok {
                    return;
                }
                let p = path_for_delete.clone();
                let next = next_for_delete.clone();
                let prev = prev_for_delete.clone();
                spawn_local(async move {
                    match api::delete_watch_group_file(wg_id, &p).await {
                        Ok(()) => match (next, prev) {
                            (Some(n), _) => batch(move || {
                                all_files.update(|v| {
                                    v.retain(|f| f.relative_path.to_serialized_string() != p)
                                });
                                current_path.set(n);
                            }),
                            (None, Some(pr)) => batch(move || {
                                all_files.update(|v| {
                                    v.retain(|f| f.relative_path.to_serialized_string() != p)
                                });
                                current_path.set(pr);
                            }),
                            (None, None) => {
                                navigate_sv.get_value()(
                                    &format!("/app/watch-groups/{wg_id}"),
                                    Default::default(),
                                );
                            }
                        },
                        Err(e) => msg.error(e),
                    }
                });
            };

            view! {
                <div class="gallery-container">
                    <div class="gallery-top-bar">
                        <Message signal=msg />
                        <button
                            class="btn btn-icon btn-danger gallery-delete-btn"
                            title="Delete image"
                            on:click=on_delete_click
                        >
                            <TrashIcon />
                        </button>
                    </div>
                    <div class="gallery-nav-left">
                        {if let Some(prev) = prev_path {
                            view! {
                                <button
                                    class="gallery-nav-btn"
                                    on:click=move |_| current_path.set(prev.clone())
                                >
                                    "\u{2039}"
                                </button>
                            }
                            .into_any()
                        } else {
                            view! {
                                <button class="gallery-nav-btn" disabled=true>
                                    "\u{2039}"
                                </button>
                            }
                            .into_any()
                        }}
                    </div>
                    <div class="gallery-content">
                        <img class="gallery-img" src=img_src />
                        <div class="gallery-filename">{file_name}</div>
                    </div>
                    <div class="gallery-nav-right">
                        {if let Some(next) = next_path {
                            view! {
                                <button
                                    class="gallery-nav-btn"
                                    on:click=move |_| current_path.set(next.clone())
                                >
                                    "\u{203A}"
                                </button>
                            }
                            .into_any()
                        } else {
                            view! {
                                <button class="gallery-nav-btn" disabled=true>
                                    "\u{203A}"
                                </button>
                            }
                            .into_any()
                        }}
                    </div>
                </div>
            }
            .into_any()
        }}
    }
}
