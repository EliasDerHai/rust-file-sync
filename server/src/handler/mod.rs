mod app;
mod client;
mod client_watch_group;
mod config;
pub mod link;
mod pwa;
mod server_watch_group;
mod sync;

pub use app::serve_embedded_app;
pub use client::{api_delete_client, api_get_client, api_list_clients, api_update_client};
pub use client_watch_group::{
    api_create_client_watch_group, api_delete_client_watch_group, api_list_client_watch_groups,
    api_update_client_watch_group,
};
pub use config::get_config;
pub use link::{get_links, post_link, post_link_tag};
pub use pwa::serve_embedded_pwa;
pub use server_watch_group::{
    api_create_watch_group, api_delete_watch_group, api_list_watch_groups, api_update_watch_group,
};
pub use sync::{delete, download, scan_disk, sync_handler, upload_handler};

use axum::http::{HeaderMap, StatusCode};

pub(crate) fn header_value_as_opt_string(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}

pub(crate) fn header_value_as_string<'header>(
    headers: &'header HeaderMap,
    key: &str,
) -> Result<&'header str, (StatusCode, String)> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, format!("Missing {key} header")))
}
