pub mod client;
pub mod protocol;
#[cfg(unix)]
pub mod server;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::LocalLb;
use crate::model::core_config::Config;

pub const SOCKET_FILENAME: &str = "lb.sock";

pub fn socket_path(writeable_path: impl AsRef<Path>) -> PathBuf {
    writeable_path.as_ref().join(SOCKET_FILENAME)
}

pub fn spawn_host(lb: Arc<LocalLb>) {
    #[cfg(not(unix))]
    {
        let _ = lb;
    }
    #[cfg(unix)]
    {
        let socket = socket_path(&lb.config.writeable_path);
        if socket.exists() {
            let _ = std::fs::remove_file(&socket);
        }
        match tokio::net::UnixListener::bind(&socket) {
            Ok(listener) => {
                tokio::spawn(server::serve(listener, lb));
            }
            Err(err) => {
                tracing::warn!(?err, "failed to bind ipc listener; guests cannot attach");
            }
        }
    }
}

pub async fn connect_guest(config: &Config) -> Option<Arc<client::RemoteLb>> {
    #[cfg(not(unix))]
    {
        let _ = config;
        None
    }
    #[cfg(unix)]
    {
        let socket = socket_path(&config.writeable_path);
        if !socket.exists() {
            return None;
        }
        let mut attempts: u32 = 0;
        let mut delay = std::time::Duration::from_millis(10);
        loop {
            match client::RemoteLb::connect(&socket, config.clone()).await {
                Ok(c) => return Some(c),
                Err(_) if attempts < 10 => {
                    attempts += 1;
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, std::time::Duration::from_millis(500));
                }
                Err(_) => return None,
            }
        }
    }
}
