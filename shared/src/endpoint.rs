pub const CLIENT_HOST_HEADER_KEY: &str = "X-Client-Hostname";
pub const CLIENT_ID_HEADER_KEY: &str = "X-Client-Id";

pub enum ServerEndpoint {
    Hello,
    Ping,
    Version,
    Scan,

    /// SYS
    Sync,
    Upload,
    Download,
    Delete,
    Config,

    /// PWA
    ServePWA,
    /// SPA frontend
    App,

    /// Receive shared links from PWA
    ApiLinks,
    /// JSON API: list all client configs
    ApiConfigs,
    /// JSON API: get single client config
    ApiConfig,
    /// JSON API: list all watch groups
    ApiWatchGroups,
    /// JSON API: single watch group by ID
    ApiWatchGroup,
    /// JSON API: monitoring data
    ApiMonitor,
}

impl ServerEndpoint {
    pub fn to_uri(&self, base: &str) -> String {
        format!("{base}{}", self.to_str())
    }

    /// Build URI with watch group id interpolated
    pub fn to_uri_with_wg(&self, base: &str, wg_id: i64) -> String {
        self.to_uri(base).replace("{wg_id}", &wg_id.to_string())
    }

    pub fn to_str(&self) -> &str {
        match self {
            ServerEndpoint::Hello => "/",
            ServerEndpoint::Ping => "/ping",
            ServerEndpoint::Version => "/version",
            ServerEndpoint::Scan => "/scan",
            // sys
            ServerEndpoint::Sync => "/sys/sync/{wg_id}",
            ServerEndpoint::Upload => "/sys/upload/{wg_id}",
            ServerEndpoint::Download => "/sys/download/{wg_id}",
            ServerEndpoint::Delete => "/sys/delete/{wg_id}",
            ServerEndpoint::Config => "/sys/config",
            // apps
            ServerEndpoint::ServePWA => "/pwa",
            ServerEndpoint::App => "/app",
            // apps
            ServerEndpoint::ApiLinks => "/api/links",
            ServerEndpoint::ApiConfigs => "/api/configs",
            ServerEndpoint::ApiConfig => "/api/config/{id}",
            ServerEndpoint::ApiWatchGroups => "/api/watch-groups",
            ServerEndpoint::ApiWatchGroup => "/api/watch-groups/{id}",
            ServerEndpoint::ApiMonitor => "/api/monitor",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ServerEndpoint::*;

    const ALL_ENDPOINTS: [ServerEndpoint; 17] = [
        Hello,
        Ping,
        Version,
        Scan,
        Sync,
        Upload,
        Download,
        Delete,
        Config,
        ServePWA,
        App,
        ApiLinks,
        ApiConfigs,
        ApiConfig,
        ApiWatchGroups,
        ApiWatchGroup,
        ApiMonitor,
    ];

    #[test]
    fn should_build_uris() {
        ALL_ENDPOINTS.into_iter().for_each(|endpoint| {
            let actual = endpoint.to_uri("http://localhost");
            match endpoint {
                Hello => assert_eq!("http://localhost/", actual),
                Ping => assert_eq!("http://localhost/ping", actual),
                Version => assert_eq!("http://localhost/version", actual),
                Scan => assert_eq!("http://localhost/scan", actual),

                Sync => assert_eq!("http://localhost/sys/sync/{wg_id}", actual),
                Upload => assert_eq!("http://localhost/sys/upload/{wg_id}", actual),
                Download => assert_eq!("http://localhost/sys/download/{wg_id}", actual),
                Delete => assert_eq!("http://localhost/sys/delete/{wg_id}", actual),
                Config => assert_eq!("http://localhost/sys/config", actual),

                ServePWA => assert_eq!("http://localhost/pwa", actual),
                App => assert_eq!("http://localhost/app", actual),

                ApiLinks => assert_eq!("http://localhost/api/links", actual),
                ApiConfigs => assert_eq!("http://localhost/api/configs", actual),
                ApiConfig => assert_eq!("http://localhost/api/config/{id}", actual),
                ApiWatchGroups => assert_eq!("http://localhost/api/watch-groups", actual),
                ApiWatchGroup => assert_eq!("http://localhost/api/watch-groups/{id}", actual),
                ApiMonitor => assert_eq!("http://localhost/api/monitor", actual),
            }
        })
    }

    #[test]
    fn should_build_uris_with_watch_group_id() {
        assert_eq!(
            "http://localhost/sys/sync/42",
            Sync.to_uri_with_wg("http://localhost", 42)
        );
        assert_eq!(
            "http://localhost/sys/upload/1",
            Upload.to_uri_with_wg("http://localhost", 1)
        );
        assert_eq!(
            "http://localhost/sys/download/7",
            Download.to_uri_with_wg("http://localhost", 7)
        );
        assert_eq!(
            "http://localhost/sys/delete/99",
            Delete.to_uri_with_wg("http://localhost", 99)
        );
    }
}
