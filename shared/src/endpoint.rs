pub const CLIENT_HOST_HEADER_KEY: &str = "X-Client-Hostname";
pub const CLIENT_ID_HEADER_KEY: &str = "X-Client-Id";

pub enum ServerEndpoint {
    Ping,
    Sync,
    Upload,
    Download,
    Delete,
    // TODO: Config and Regsiter should be same endpoint
    /// Temporary endpoint for migrating client configs to server DB
    Register,
    /// Get client config from server
    Config,
    
    /// not used from client - can be directly accessed from browser etc. for inspection
    Scan,
    Monitor,
    Version,
    Hello,
    /// Get/set client config (admin UI)
    AdminConfig,
    /// List all client configs (admin UI)
    AdminConfigs,
}

impl ServerEndpoint {
    /// endpoint without ssl
    pub fn to_uri(&self, base: &str) -> String {
        format!("http://{base}{}", self.to_str())
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
            ServerEndpoint::Register => "/register",
            ServerEndpoint::Config => "/config",
            ServerEndpoint::AdminConfig => "/admin/config/{id}",
            ServerEndpoint::AdminConfigs => "/admin/configs",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ServerEndpoint::*;

    const ALL_ENDPOINTS: [ServerEndpoint; 12] = [
        Ping,
        Scan,
        Sync,
        Upload,
        Download,
        Delete,
        Monitor,
        Version,
        Register,
        Config,
        AdminConfig,
        AdminConfigs,
    ];

    #[test]
    fn should_build_uris() {
        ALL_ENDPOINTS.into_iter().for_each(|endpoint| {
            let actual = endpoint.to_uri("localhost");
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
                Register => assert_eq!("http://localhost/register", actual),
                Config => assert_eq!("http://localhost/config", actual),
                AdminConfig => assert_eq!("http://localhost/admin/config/:id", actual),
                AdminConfigs => assert_eq!("http://localhost/admin/configs", actual),
            }
        })
    }
}
