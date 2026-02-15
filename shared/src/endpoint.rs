pub const CLIENT_HOST_HEADER_KEY: &str = "X-Client-Hostname";
pub const CLIENT_ID_HEADER_KEY: &str = "X-Client-Id";

pub enum ServerEndpoint {
    Hello,
    Ping,
    Version,
    Scan,

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

    pub fn to_str(&self) -> &str {
        match self {
            ServerEndpoint::Hello => "/",
            ServerEndpoint::Ping => "/ping",
            ServerEndpoint::Version => "/version",
            ServerEndpoint::Scan => "/scan",
            // sys
            ServerEndpoint::Sync => "/sys/sync",
            ServerEndpoint::Upload => "/sys/upload",
            ServerEndpoint::Download => "/sys/download",
            ServerEndpoint::Delete => "/sys/delete",
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

                Sync => assert_eq!("http://localhost/sys/sync", actual),
                Upload => assert_eq!("http://localhost/sys/upload", actual),
                Download => assert_eq!("http://localhost/sys/download", actual),
                Delete => assert_eq!("http://localhost/sys/delete", actual),
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
}
