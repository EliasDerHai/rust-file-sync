pub enum ServerEndpoint {
    Ping,
    Sync,
    Upload,
    Download,
    Delete,
    // not used from client - can be directly accessed from browser etc. for inspection
    Scan, 
    Monitor,
}

impl ServerEndpoint {
    /// endpoint without ssl
    pub fn to_uri(&self, base: &str) -> String {
        format!("http://{base}{}", self.to_str())
    }

    pub fn to_str(&self) -> &str {
        match self {
            ServerEndpoint::Ping => "/ping",
            ServerEndpoint::Scan => "/scan",
            ServerEndpoint::Sync => "/sync",
            ServerEndpoint::Upload => "/upload",
            ServerEndpoint::Download => "/download",
            ServerEndpoint::Delete => "/delete",
            ServerEndpoint::Monitor => "/monitor",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ServerEndpoint::*;

    const ALL_ENDPOINTS: [ServerEndpoint; 7] =
        [Ping, Scan, Sync, Upload, Download, Delete, Monitor];

    #[test]
    fn should_build_uris() {
        ALL_ENDPOINTS.into_iter().for_each(|endpoint| {
            let actual = endpoint.to_uri("localhost");
            match endpoint {
                Ping => assert_eq!("http://localhost/ping", actual),
                Scan => assert_eq!("http://localhost/scan", actual),
                Sync => assert_eq!("http://localhost/sync", actual),
                Upload => assert_eq!("http://localhost/upload", actual),
                Download => assert_eq!("http://localhost/download", actual),
                Delete => assert_eq!("http://localhost/delete", actual),
                Monitor => assert_eq!("http://localhost/monitor", actual),
            }
        })
    }
}
