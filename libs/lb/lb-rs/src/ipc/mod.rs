pub mod client;
pub mod protocol;
#[cfg(unix)]
pub mod server;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ipc::client::RemoteCallError;
use crate::ipc::protocol::Request;
use crate::model::core_config::Config;
use crate::model::errors::{LbErrKind, LbResult};
use crate::{Lb, LocalLb};

pub const SOCKET_FILENAME: &str = "lb.sock";

pub fn socket_path(writeable_path: impl AsRef<Path>) -> PathBuf {
    writeable_path.as_ref().join(SOCKET_FILENAME)
}

impl Lb {
    pub fn is_local(&self) -> bool {
        self.local.get().is_some()
    }

    pub fn try_local(&self) -> Option<LocalLb> {
        self.local.get().cloned()
    }

    pub(crate) async fn recover(&self) -> LbResult<LocalLb> {
        if let Some(local) = self.local.get() {
            return Ok(local.clone());
        }
        let loc = match LocalLb::init(self.config.clone()).await {
            Ok(l) => l,
            Err(e) => {
                if let Some(local) = self.local.get() {
                    return Ok(local.clone());
                }
                return Err(e);
            }
        };
        spawn_host(loc.clone());
        match self.local.set(loc.clone()) {
            Ok(()) => Ok(loc),
            Err(_) => Ok(self.local.get().unwrap().clone()),
        }
    }

    pub async fn call<Out>(&self, req: Request) -> LbResult<Out>
    where
        Out: serde::de::DeserializeOwned,
    {
        let remote = self
            .remote
            .as_ref()
            .expect("Lb::call: remote must be set when local is unset");
        match remote.try_call::<Out>(req.clone()).await {
            Ok(v) => Ok(v),
            #[cfg(unix)]
            Err(RemoteCallError::HostUnavailable) => {
                let local = self.recover().await?;
                let bytes = server::dispatch(&local, req).await;
                let result: LbResult<Out> = bincode::deserialize(&bytes).map_err(|e| {
                    LbErrKind::Unexpected(format!("local dispatch deserialize: {e}"))
                })?;
                result
            }
            #[cfg(not(unix))]
            Err(RemoteCallError::HostUnavailable) => {
                let _ = req;
                unreachable!("HostUnavailable cannot occur on non-unix targets")
            }
            Err(RemoteCallError::Other(e)) => Err(e),
        }
    }
}

pub fn spawn_host(lb: LocalLb) {
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
