use leptos::prelude::*;

use crate::api;
use crate::components::{EmptyState, FileIcon, Loading};
use crate::format::format_size;

#[component]
pub fn BackupsPage() -> impl IntoView {
    let backups = LocalResource::new(api::fetch_backups);

    view! {
        <div class="container">
            <h1>"Backups"</h1>

            <Suspense fallback=Loading>
                {move || Suspend::new(async move {
                    match backups.await {
                        Err(e) => view! { <div class="message message-error">"Error: " {e}</div> }.into_any(),
                        Ok(backups) => {
                            if backups.is_empty() {
                                view! { <EmptyState message="No backups yet." /> }.into_any()
                            } else {
                                view! {
                                    <ul class="filetree-list">
                                        {backups.into_iter().map(|backup| {
                                            let size = format_size(backup.size_in_bytes);
                                            let created_at = backup.created_at_utc_millis.to_string();
                                            let download_url = api::backup_download_url(backup.index);
                                            view! {
                                                <li>
                                                    <a
                                                        class="filetree-row"
                                                        href=download_url
                                                        target="_blank"
                                                    >
                                                        <FileIcon />
                                                        <span>{backup.file_name}</span>
                                                        <span class="filetree-row-meta">
                                                            <span>{created_at}</span>
                                                            <span>{size}</span>
                                                        </span>
                                                    </a>
                                                </li>
                                            }
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
