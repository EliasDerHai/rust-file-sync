mod config;
mod pwa;
mod share_link;
mod sync;

pub use config::admin::{get_config_edit, list_configs, update_config};
pub use config::client::{get_config, post_config};
pub use pwa::serve_embedded_pwa;
pub use share_link::receive_shared_link;
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
