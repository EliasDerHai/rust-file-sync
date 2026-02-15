mod app;
mod config;
mod pwa;
pub mod share_link;
mod sync;
mod watch_group;

pub use app::serve_embedded_app;
pub use config::admin::{api_get_config, api_list_configs, api_update_config};
pub use config::client::{get_config, post_config};
pub use pwa::serve_embedded_pwa;
pub use share_link::{get_links, post_link};
pub use sync::{delete, download, scan_disk, sync_handler, upload_handler};
pub use watch_group::{api_create_watch_group, api_list_watch_groups, api_update_watch_group};

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
