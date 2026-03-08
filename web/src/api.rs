use gloo_net::http::Request;
use shared::{
    dtos::{
        ClientDto, ClientUpdateDto, ClientWatchGroupCreateDto, ClientWatchGroupDto,
        ClientWatchGroupUpdateDto, MonitorData, ServerWatchGroup, WatchGroupNameDto,
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

// pub async fn fetch_configs() -> Result<Vec<ClientWithConfig>, String> {
//     Request::get(ServerEndpoint::ApiConfigs.to_str())
//         .send()
//         .await
//         .map_err(|e| e.to_string())?
//         .json()
//         .await
//         .map_err(|e| e.to_string())
// }
//
// pub async fn fetch_config(id: &str) -> Result<ClientWithConfig, String> {
//     Request::get(&ServerEndpoint::ApiConfig.to_str().replace("{id}", id))
//         .send()
//         .await
//         .map_err(|e| e.to_string())?
//         .json()
//         .await
//         .map_err(|e| e.to_string())
// }
//
// pub async fn update_config(id: &str, dto: &ClientWatchGroupUpdateDto) -> Result<String, String> {
//     let resp = Request::put(&ServerEndpoint::ApiConfig.to_str().replace("{id}", id))
//         .json(dto)
//         .map_err(|e| e.to_string())?
//         .send()
//         .await
//         .map_err(|e| e.to_string())?;
//     let text = resp.text().await.map_err(|e| e.to_string())?;
//     if resp.ok() { Ok(text) } else { Err(text) }
// }

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

pub async fn fetch_client_watch_groups(client_id: &str) -> Result<Vec<ClientWatchGroupDto>, String> {
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

pub async fn fetch_monitor_data() -> Result<MonitorData, String> {
    Request::get(ServerEndpoint::ApiMonitor.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}
