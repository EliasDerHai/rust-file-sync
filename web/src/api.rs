use gloo_net::http::Request;

use crate::types::*;

pub async fn fetch_configs() -> Result<Vec<ClientWithConfig>, String> {
    Request::get("/api/configs")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_config(id: &str) -> Result<ClientWithConfig, String> {
    Request::get(&format!("/api/config/{}", id))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn update_config(id: &str, dto: &AdminConfigUpdateDto) -> Result<String, String> {
    let resp = Request::put(&format!("/api/config/{}", id))
        .json(dto)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(text)
    } else {
        Err(text)
    }
}

pub async fn fetch_watch_groups() -> Result<Vec<ServerWatchGroup>, String> {
    Request::get("/api/watch-groups")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn create_watch_group(dto: &WatchGroupNameDto) -> Result<String, String> {
    let resp = Request::post("/api/watch-groups")
        .json(dto)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(text)
    } else {
        Err(text)
    }
}

pub async fn update_watch_group(id: i64, dto: &WatchGroupNameDto) -> Result<String, String> {
    let resp = Request::put(&format!("/api/watch-groups/{}", id))
        .json(dto)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(text)
    } else {
        Err(text)
    }
}

pub async fn fetch_monitor_data() -> Result<MonitorData, String> {
    Request::get("/api/monitor")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}
