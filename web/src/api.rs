use gloo_net::http::Request;
use shared::{
    dtos::{
        ClientWatchGroupUpdateDto, ClientWithConfig, MonitorData, ServerWatchGroup,
        WatchGroupNameDto,
    },
    endpoint::ServerEndpoint,
};

pub async fn fetch_configs() -> Result<Vec<ClientWithConfig>, String> {
    Request::get(ServerEndpoint::ApiConfigs.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_config(id: &str) -> Result<ClientWithConfig, String> {
    Request::get(&ServerEndpoint::ApiConfig.to_str().replace("{id}", id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn update_config(id: &str, dto: &ClientWatchGroupUpdateDto) -> Result<String, String> {
    let resp = Request::put(&ServerEndpoint::ApiConfig.to_str().replace("{id}", id))
        .json(dto)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(text) } else { Err(text) }
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

pub async fn fetch_monitor_data() -> Result<MonitorData, String> {
    Request::get(ServerEndpoint::ApiMonitor.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}
