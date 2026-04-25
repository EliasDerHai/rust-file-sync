use gloo_net::http::Request;
use shared::{
    dtos::{
        ClientDto, ClientUpdateDto, ClientWatchGroupCreateDto, ClientWatchGroupDto,
        ClientWatchGroupUpdateDto, FileDescription, LinkCreateDto, LinkDeleteDto, LinkDto,
        MonitorData, ServerWatchGroup, WatchGroupNameDto,
    },
    endpoint::ServerEndpoint,
};

pub async fn fetch_clients() -> Result<Vec<ClientDto>, String> {
    Request::get(ServerEndpoint::ApiClients.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_watch_groups() -> Result<Vec<ServerWatchGroup>, String> {
    Request::get(ServerEndpoint::ApiWatchGroups.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn create_watch_group(dto: &WatchGroupNameDto) -> Result<String, String> {
    let resp = Request::post(ServerEndpoint::ApiWatchGroups.to_str())
        .json(dto)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(text) } else { Err(text) }
}

pub async fn update_watch_group(id: i64, dto: &WatchGroupNameDto) -> Result<String, String> {
    let resp = Request::put(
        &ServerEndpoint::ApiWatchGroup
            .to_str()
            .replace("{id}", &id.to_string()),
    )
    .json(dto)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(text) } else { Err(text) }
}

pub async fn fetch_client_watch_groups(
    client_id: &str,
) -> Result<Vec<ClientWatchGroupDto>, String> {
    Request::get(
        &ServerEndpoint::ApiClientWatchGroups
            .to_str()
            .replace("{id}", client_id),
    )
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())
}

pub async fn update_client(client_id: &str, dto: &ClientUpdateDto) -> Result<(), String> {
    let resp = Request::put(
        &ServerEndpoint::ApiClient
            .to_str()
            .replace("{id}", client_id),
    )
    .json(dto)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(text) }
}

pub async fn delete_client(client_id: &str) -> Result<(), String> {
    let resp = Request::delete(
        &ServerEndpoint::ApiClient
            .to_str()
            .replace("{id}", client_id),
    )
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(text) }
}

pub async fn create_client_watch_group(
    client_id: &str,
    dto: &ClientWatchGroupCreateDto,
) -> Result<(), String> {
    let resp = Request::post(
        &ServerEndpoint::ApiClientWatchGroups
            .to_str()
            .replace("{id}", client_id),
    )
    .json(dto)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(text) }
}

pub async fn update_client_watch_group(
    client_id: &str,
    wg_id: i64,
    dto: &ClientWatchGroupUpdateDto,
) -> Result<(), String> {
    let resp = Request::put(
        &ServerEndpoint::ApiClientWatchGroup
            .to_str()
            .replace("{id}", client_id)
            .replace("{wg_id}", &wg_id.to_string()),
    )
    .json(dto)
    .map_err(|e| e.to_string())?
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(text) }
}

pub async fn delete_client_watch_group(client_id: &str, wg_id: i64) -> Result<(), String> {
    let resp = Request::delete(
        &ServerEndpoint::ApiClientWatchGroup
            .to_str()
            .replace("{id}", client_id)
            .replace("{wg_id}", &wg_id.to_string()),
    )
    .send()
    .await
    .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(text) }
}

pub async fn fetch_watch_group_files(wg_id: i64) -> Result<Vec<FileDescription>, String> {
    Request::get(
        &ServerEndpoint::ApiWatchGroupFiles
            .to_str()
            .replace("{id}", &wg_id.to_string()),
    )
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())
}

pub fn watch_group_file_preview_url(wg_id: i64, path: &str) -> String {
    let encoded = js_sys::encode_uri_component(path);
    format!(
        "/api/watch-groups/{}/file?path={}",
        wg_id,
        String::from(encoded)
    )
}

pub fn gallery_url(wg_id: i64, path: &str) -> String {
    let encoded = js_sys::encode_uri_component(path);
    format!(
        "/app/watch-groups/{}/gallery?path={}",
        wg_id,
        String::from(encoded)
    )
}

pub async fn fetch_monitor_data() -> Result<MonitorData, String> {
    Request::get(ServerEndpoint::ApiMonitor.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

// links

pub async fn fetch_links() -> Result<Vec<LinkDto>, String> {
    Request::get(ServerEndpoint::ApiLinks.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn create_link(dto: LinkCreateDto) -> Result<(), String> {
    Request::post(ServerEndpoint::ApiLinks.to_str())
        .json(&dto)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn delete_link(url: &str) -> Result<(), String> {
    let resp = Request::delete(ServerEndpoint::ApiLinks.to_str())
        .json(&LinkDeleteDto {
            url: url.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(text) }
}
