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

    /// Shared links from PWA
    ApiLinks,
    /// Tags for links
    ApiLinkTags,
    /// JSON API: list / create clients
    ApiClients,
    /// JSON API: single client (GET, PUT, DELETE)
    ApiClient,
    /// JSON API: list / create client watch group assignments
    ApiClientWatchGroups,
    /// JSON API: single client watch group assignment (PUT, DELETE)
    ApiClientWatchGroup,
    /// JSON API: list / create server watch groups
    ApiWatchGroups,
    /// JSON API: single server watch group (PUT, DELETE)
    ApiWatchGroup,
    /// JSON API: list / create files within one watch group
    ApiWatchGroupFiles,
    /// Inline file preview for one watch group file
    ApiWatchGroupFile,
    /// JSON API: monitoring data
    ApiMonitor,
}

impl ServerEndpoint {
    pub fn to_uri(&self, base: &str) -> String {
        format!("{base}{}", self.to_str())
    }

    /// Build URI with a set of named path parameter replacements.
    /// e.g. `endpoint.to_uri_with(base, &[("id", "abc"), ("wg_id", "1")])`
    pub fn to_uri_with(&self, base: &str, params: &[(&str, &str)]) -> String {
        let mut uri = self.to_uri(base);
        for (key, value) in params {
            uri = uri.replace(&format!("{{{key}}}"), value);
        }
        uri
    }

    /// Build URI with watch group id interpolated (convenience for sys endpoints)
    pub fn to_uri_with_wg(&self, base: &str, wg_id: i64) -> String {
        self.to_uri_with(base, &[("wg_id", &wg_id.to_string())])
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
            // api
            ServerEndpoint::ApiLinks => "/api/links",
            ServerEndpoint::ApiLinkTags => "/api/links/tags",
            ServerEndpoint::ApiClients => "/api/clients",
            ServerEndpoint::ApiClient => "/api/clients/{id}",
            ServerEndpoint::ApiClientWatchGroups => "/api/clients/{id}/watch-groups",
            ServerEndpoint::ApiClientWatchGroup => "/api/clients/{id}/watch-groups/{wg_id}",
            ServerEndpoint::ApiWatchGroups => "/api/watch-groups",
            ServerEndpoint::ApiWatchGroup => "/api/watch-groups/{id}",
            ServerEndpoint::ApiWatchGroupFiles => "/api/watch-groups/{id}/files",
            ServerEndpoint::ApiWatchGroupFile => "/api/watch-groups/{id}/file",
            ServerEndpoint::ApiMonitor => "/api/monitor",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ServerEndpoint::*;

    const ALL_ENDPOINTS: [ServerEndpoint; 21] = [
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
        ApiClients,
        ApiClient,
        ApiClientWatchGroups,
        ApiClientWatchGroup,
        ApiWatchGroups,
        ApiWatchGroup,
        ApiWatchGroupFiles,
        ApiWatchGroupFile,
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
                ApiLinkTags => assert_eq!("http://localhost/api/links/tags", actual),
                ApiClients => assert_eq!("http://localhost/api/clients", actual),
                ApiClient => assert_eq!("http://localhost/api/clients/{id}", actual),
                ApiClientWatchGroups => {
                    assert_eq!("http://localhost/api/clients/{id}/watch-groups", actual)
                }
                ApiClientWatchGroup => assert_eq!(
                    "http://localhost/api/clients/{id}/watch-groups/{wg_id}",
                    actual
                ),
                ApiWatchGroups => assert_eq!("http://localhost/api/watch-groups", actual),
                ApiWatchGroup => assert_eq!("http://localhost/api/watch-groups/{id}", actual),
                ApiWatchGroupFiles => {
                    assert_eq!("http://localhost/api/watch-groups/{id}/files", actual)
                }
                ApiWatchGroupFile => {
                    assert_eq!("http://localhost/api/watch-groups/{id}/file", actual)
                }
                ApiMonitor => assert_eq!("http://localhost/api/monitor", actual),
            }
        })
    }

    #[test]
    fn should_build_uris_with_params() {
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
