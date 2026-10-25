use gloo_net::http::Request;
use shared::{
    dtos::{
        BackupFileDto, ClientDto, ClientUpdateDto, ClientWatchGroupCreateDto, ClientWatchGroupDto,
        ClientWatchGroupUpdateDto, FileDescription, LinkCreateDto, LinkDeleteDto, LinkDto,
        LocationPointDeleteDto, LocationPointDto, LogLineDto, MapConfigDto, MonitorSeriesDto,
        ServerWatchGroup, WatchGroupNameDto,
    },
    endpoint::{CLIENT_ID_HEADER_KEY, ServerEndpoint, WEB_CLIENT_ID},
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

/// The server's own version (plain text), used to flag outdated clients.
pub async fn fetch_server_version() -> Result<String, String> {
    Request::get(ServerEndpoint::Version.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())
        .map(|v| v.trim().to_string())
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

pub async fn delete_watch_group_file(wg_id: i64, path: &str) -> Result<(), String> {
    let encoded = js_sys::encode_uri_component(path);
    let url = format!(
        "{}?path={}",
        ServerEndpoint::ApiWatchGroupFile
            .to_str()
            .replace("{id}", &wg_id.to_string()),
        String::from(encoded)
    );
    let resp = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if resp.ok() { Ok(()) } else { Err(text) }
}

/// Uploads one file into `dir` (`/`-separated, relative to the watch group root; may be empty).
/// An existing file with the same name is replaced.
pub async fn upload_watch_group_file(
    wg_id: i64,
    dir: &str,
    file: &web_sys::File,
) -> Result<(), String> {
    let form = web_sys::FormData::new().map_err(|e| format!("{e:?}"))?;
    form.append_with_str("dir", dir)
        .map_err(|e| format!("{e:?}"))?;
    form.append_with_blob_and_filename("file", file, &file.name())
        .map_err(|e| format!("{e:?}"))?;
    let resp = Request::post(
        &ServerEndpoint::ApiWatchGroupFiles
            .to_str()
            .replace("{id}", &wg_id.to_string()),
    )
    .header(CLIENT_ID_HEADER_KEY, WEB_CLIENT_ID)
    .body(form)
    .map_err(|e| e.to_string())?
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

/// Small server-generated JPEG for tile previews; `mtime` only busts the browser cache
/// (the server sends a long `Cache-Control`), so a changed file gets a new URL.
pub fn watch_group_file_thumbnail_url(wg_id: i64, path: &str, mtime: u64) -> String {
    let encoded = js_sys::encode_uri_component(path);
    format!(
        "/api/watch-groups/{}/thumbnail?path={}&m={}",
        wg_id,
        String::from(encoded),
        mtime
    )
}

/// Monitoring buckets for `[from_ms, to_ms)`; the server picks the tier (5s / 1m / 1h)
/// and bucket size so roughly `points` buckets come back.
pub async fn fetch_monitor(
    from_ms: i64,
    to_ms: i64,
    points: u32,
) -> Result<MonitorSeriesDto, String> {
    let url = format!(
        "{}?from={from_ms}&to={to_ms}&points={points}",
        ServerEndpoint::ApiMonitor.to_str()
    );
    let resp = Request::get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(resp
            .text()
            .await
            .unwrap_or_else(|_| resp.status().to_string()));
    }
    resp.json().await.map_err(|e| e.to_string())
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

// backups

pub async fn fetch_backups() -> Result<Vec<BackupFileDto>, String> {
    Request::get(ServerEndpoint::ApiBackups.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub fn backup_download_url(index: u8) -> String {
    ServerEndpoint::ApiBackup
        .to_str()
        .replace("{index}", &index.to_string())
}

// logs

pub async fn fetch_logs(tail: usize) -> Result<Vec<LogLineDto>, String> {
    Request::get(&format!(
        "{}?tail={}",
        ServerEndpoint::ApiLogs.to_str(),
        tail
    ))
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())
}

/// Lines with `seq` greater than `seq`, for resuming a paused live stream with no gaps.
pub async fn fetch_logs_since(seq: u64) -> Result<Vec<LogLineDto>, String> {
    Request::get(&format!(
        "{}?since_seq={}",
        ServerEndpoint::ApiLogs.to_str(),
        seq
    ))
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json()
    .await
    .map_err(|e| e.to_string())
}

// locations

/// Active (non-excluded) points in `[since_epoch_ms, until_epoch_ms]` (either bound
/// omitted means unbounded on that side - e.g. both omitted for "all time").
pub async fn fetch_location_points(
    since_epoch_ms: Option<i64>,
    until_epoch_ms: Option<i64>,
) -> Result<Vec<LocationPointDto>, String> {
    let mut params = Vec::new();
    if let Some(since) = since_epoch_ms {
        params.push(format!("since_epoch_ms={since}"));
    }
    if let Some(until) = until_epoch_ms {
        params.push(format!("until_epoch_ms={until}"));
    }
    let mut url = ServerEndpoint::ApiLocations.to_str().to_string();
    if !params.is_empty() {
        url = format!("{url}?{}", params.join("&"));
    }

    Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Bulk soft-delete (exclude) user-confirmed outlier points. Returns the number
/// actually excluded.
pub async fn soft_delete_location_points(ids: &[i64]) -> Result<usize, String> {
    let resp = Request::delete(ServerEndpoint::ApiLocations.to_str())
        .json(&LocationPointDeleteDto { ids: ids.to_vec() })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        resp.json().await.map_err(|e| e.to_string())
    } else {
        Err(resp.text().await.unwrap_or_default())
    }
}

/// MapTiler config (API key) for the Locations map - `maptiler_key` is `None` when
/// the server has no `MAPTILER_API_KEY` configured.
pub async fn fetch_map_config() -> Result<MapConfigDto, String> {
    Request::get(ServerEndpoint::ApiMapConfig.to_str())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}
