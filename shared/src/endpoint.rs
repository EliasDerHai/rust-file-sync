pub const CLIENT_HOST_HEADER_KEY: &str = "X-Client-Hostname";
pub const CLIENT_ID_HEADER_KEY: &str = "X-Client-Id";

pub enum ServerEndpoint {
    Ping,
    Sync,
    Upload,
    Download,
    Delete,
    /// Get/register client config
    Config,

    /// not used from client - can be directly accessed from browser etc. for inspection
    Scan,
    Monitor,
    Version,
    Hello,

    #[deprecated]
    /// Get/set server-watch-group (admin UI)
    AdminWatchGroup,
    #[deprecated]
    /// List all server-watch-groups (admin UI)
    AdminWatchGroups,
    #[deprecated]
    /// Get/set client config (admin UI)
    AdminConfig,
    #[deprecated]
    /// List all client configs (admin UI)
    AdminConfigs,

    /// PWA
    ServePWA,
    /// Receive shared links from PWA
    ShareLink,

    /// JSON API: list all client configs
    ApiConfigs,
    /// JSON API: get single client config
    ApiConfig,
    /// JSON API: list all watch groups
    ApiWatchGroups,
    /// JSON API: monitoring data
    ApiMonitor,
    /// SPA frontend
    App,
}

impl ServerEndpoint {
    pub fn to_uri(&self, base: &str) -> String {
        format!("{base}{}", self.to_str())
    }

    pub fn to_str(&self) -> &str {
        match self {
            ServerEndpoint::Hello => "/",
            ServerEndpoint::Ping => "/ping",
            ServerEndpoint::Scan => "/scan",
            ServerEndpoint::Sync => "/sync",
            ServerEndpoint::Upload => "/upload",
            ServerEndpoint::Download => "/download",
            ServerEndpoint::Delete => "/delete",
            ServerEndpoint::Monitor => "/monitor",
            ServerEndpoint::Version => "/version",
            ServerEndpoint::Config => "/config",
            ServerEndpoint::AdminConfig => "/admin/config/{id}",
            ServerEndpoint::AdminConfigs => "/admin/configs",
            ServerEndpoint::AdminWatchGroup => "/admin/watch-group/{id}",
            ServerEndpoint::AdminWatchGroups => "/admin/watch-groups",
            ServerEndpoint::ServePWA => "/pwa",
            ServerEndpoint::ShareLink => "/share-link",
            ServerEndpoint::ApiConfigs => "/api/configs",
            ServerEndpoint::ApiConfig => "/api/config/{id}",
            ServerEndpoint::ApiWatchGroups => "/api/watch-groups",
            ServerEndpoint::ApiMonitor => "/api/monitor",
            ServerEndpoint::App => "/app",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ServerEndpoint::*;

    const ALL_ENDPOINTS: [ServerEndpoint; 20] = [
        Ping,
        Scan,
        Sync,
        Upload,
        Download,
        Delete,
        Monitor,
        Version,
        Config,
        AdminConfig,
        AdminConfigs,
        AdminWatchGroup,
        AdminWatchGroups,
        ServePWA,
        ShareLink,
        ApiConfigs,
        ApiConfig,
        ApiWatchGroups,
        ApiMonitor,
        App,
    ];

    #[test]
    fn should_build_uris() {
        ALL_ENDPOINTS.into_iter().for_each(|endpoint| {
            let actual = endpoint.to_uri("http://localhost");
            match endpoint {
                Hello => assert_eq!("http://localhost/", actual),
                Ping => assert_eq!("http://localhost/ping", actual),
                Scan => assert_eq!("http://localhost/scan", actual),
                Sync => assert_eq!("http://localhost/sync", actual),
                Upload => assert_eq!("http://localhost/upload", actual),
                Download => assert_eq!("http://localhost/download", actual),
                Delete => assert_eq!("http://localhost/delete", actual),
                Monitor => assert_eq!("http://localhost/monitor", actual),
                Version => assert_eq!("http://localhost/version", actual),
                Config => assert_eq!("http://localhost/config", actual),
                AdminConfig => assert_eq!("http://localhost/admin/config/{id}", actual),
                AdminConfigs => assert_eq!("http://localhost/admin/configs", actual),
                AdminWatchGroup => assert_eq!("http://localhost/admin/watch-group/{id}", actual),
                AdminWatchGroups => assert_eq!("http://localhost/admin/watch-groups", actual),
                ServePWA => assert_eq!("http://localhost/pwa", actual),
                ShareLink => assert_eq!("http://localhost/share-link", actual),
                ApiConfigs => assert_eq!("http://localhost/api/configs", actual),
                ApiConfig => assert_eq!("http://localhost/api/config/{id}", actual),
                ApiWatchGroups => assert_eq!("http://localhost/api/watch-groups", actual),
                ApiMonitor => assert_eq!("http://localhost/api/monitor", actual),
                App => assert_eq!("http://localhost/app", actual),
            }
        })
    }
}
