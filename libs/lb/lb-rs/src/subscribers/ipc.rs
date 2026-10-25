use std::fs::{self, DirBuilder};
use std::io::{self, ErrorKind};
#[cfg(target_vendor = "apple")]
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use db_rs::View;
use tokio::net::UnixDatagram as AsyncUnixDatagram;
use tokio::runtime::Handle;
use tokio::task;

use crate::io::CoreDb;
use crate::model::errors::Unexpected;
use crate::{Lb, LbResult};

pub(crate) struct Ipc {
    directory: PathBuf,
    path: Option<PathBuf>,
    socket: UnixDatagram,
}

impl Ipc {
    pub(crate) fn new(db: &CoreDb, listen: bool) -> io::Result<Self> {
        let directory = db.log().directory.canonicalize()?.join("ipc");
        DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&directory)?;
        let (path, socket) = if listen {
            let mut index = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_millis();
            loop {
                let path = directory.join(index.to_string());
                #[cfg(target_vendor = "apple")]
                let bound = {
                    let socket = UnixDatagram::unbound()?;
                    with_address(&path, |address, len| unsafe {
                        libc::bind(socket.as_raw_fd(), address, len) as isize
                    })
                    .map(|_| socket)
                };
                #[cfg(not(target_vendor = "apple"))]
                let bound = UnixDatagram::bind(&path);
                match bound {
                    Ok(bound) => break (Some(path), bound),
                    Err(error)
                        if matches!(
                            error.kind(),
                            ErrorKind::AddrInUse | ErrorKind::AlreadyExists
                        ) =>
                    {
                        index = index
                            .checked_add(1)
                            .ok_or_else(|| io::Error::other("no available IPC socket names"))?;
                    }
                    Err(error) => return Err(error),
                }
            }
        } else {
            (None, UnixDatagram::unbound()?)
        };
        let ipc = Self { directory, path, socket };
        ipc.socket.set_nonblocking(true)?;
        Ok(ipc)
    }

    pub(crate) fn notify_peers(&self) {
        if let Err(error) = self.send() {
            warn!(?error, "could not notify other database instances");
        }
    }

    fn send(&self) -> io::Result<()> {
        for peer in fs::read_dir(&self.directory)? {
            let peer = peer?;
            let path = peer.path();
            if self.path.as_ref() == Some(&path)
                || !peer.file_type().is_ok_and(|kind| kind.is_socket())
            {
                continue;
            }
            #[cfg(target_vendor = "apple")]
            let sent = with_address(&path, |address, len| unsafe {
                libc::sendto(self.socket.as_raw_fd(), [1u8].as_ptr().cast(), 1, 0, address, len)
            });
            #[cfg(not(target_vendor = "apple"))]
            let sent = self.socket.send_to(&[1], &path);
            match sent {
                Ok(_) => {}
                // A queued wake already tells a slow peer to read everything it has missed.
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) if error.kind() == ErrorKind::ConnectionRefused => {
                    let _ = fs::remove_file(&path);
                }
                Err(error) => warn!(?error, ?path, "could not wake database peer"),
            }
        }
        Ok(())
    }
}

// Darwin accepts addresses up to SOCK_MAXADDRLEN, beyond sockaddr_un's fixed buffer.
// App-group paths (especially in the simulator) need that extra space.
#[cfg(target_vendor = "apple")]
fn with_address(
    path: &std::path::Path, operation: impl FnOnce(*const libc::sockaddr, libc::socklen_t) -> isize,
) -> io::Result<usize> {
    let path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
    let len = path.as_bytes_with_nul().len() + 2;
    if len > libc::SOCK_MAXADDRLEN as usize {
        return Err(io::Error::from_raw_os_error(libc::ENAMETOOLONG));
    }
    let mut address = vec![len as u8, libc::AF_UNIX as u8];
    address.extend_from_slice(path.as_bytes_with_nul());
    let result = operation(address.as_ptr().cast(), len as libc::socklen_t);
    if result < 0 { Err(io::Error::last_os_error()) } else { Ok(result as usize) }
}

impl Drop for Ipc {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = fs::remove_file(path);
        }
    }
}

impl Lb {
    pub(crate) async fn setup_ipc(&self) -> LbResult<()> {
        if !self.config.background_work {
            return Ok(());
        }
        let socket = AsyncUnixDatagram::from_std(self.ipc.socket.try_clone()?)?;

        // The socket is registered before catching up, so startup cannot miss a commit.
        self.catch_up_from_ipc().await?;

        let core = self.clone();
        tokio::spawn(async move {
            let mut message = [0];
            loop {
                if let Err(error) = socket.recv(&mut message).await {
                    error!(?error, "database notification socket failed");
                    break;
                }
                // Coalesce wake-ups before reading the log, never after it.
                while socket.try_recv(&mut message).is_ok() {}
                if let Err(error) = core.catch_up_from_ipc().await {
                    error!(?error, "database catch-up worker failed");
                    break;
                }
            }
        });
        Ok(())
    }

    async fn catch_up_from_ipc(&self) -> LbResult<()> {
        let core = self.clone();
        let runtime = Handle::current();
        // Waiting for another process's database lock must not block a Tokio worker.
        task::spawn_blocking(move || runtime.block_on(async { core.begin_tx().await.end() }))
            .await
            .map_unexpected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db_rs::config::Config;
    use tempfile::tempdir_in;

    #[cfg(target_vendor = "apple")]
    #[test]
    fn notifies_peers_with_long_app_group_paths() {
        let temporary = tempdir_in("/tmp").unwrap();
        let directory = temporary.path().join("a".repeat(150));
        fs::create_dir(&directory).unwrap();
        let db = CoreDb::init(&Config::default().log_location(&directory)).unwrap();
        let first = Ipc::new(&db, true).unwrap();
        let second = Ipc::new(&db, true).unwrap();
        first.send().unwrap();
        assert_eq!(second.socket.recv(&mut [0]).unwrap(), 1);
        second.send().unwrap();
        assert_eq!(first.socket.recv(&mut [0]).unwrap(), 1);
    }

    #[test]
    fn sends_to_peers_skips_self_and_cleans_up_dead_sockets() {
        let directory = tempdir_in("/tmp").unwrap();
        let db = CoreDb::init(&Config::default().log_location(directory.path())).unwrap();
        let ipc = Ipc::new(&db, true).unwrap();
        let peer = UnixDatagram::bind(ipc.directory.join("peer.sock")).unwrap();
        peer.set_nonblocking(true).unwrap();
        let stale = ipc.directory.join("stale.sock");
        drop(UnixDatagram::bind(&stale).unwrap());

        ipc.send().unwrap();

        assert_eq!(peer.recv(&mut [0]).unwrap(), 1);
        assert_eq!(ipc.socket.recv(&mut [0]).unwrap_err().kind(), ErrorKind::WouldBlock);
        assert!(!stale.exists());
        let path = ipc.path.clone().unwrap();
        drop(ipc);
        assert!(!path.exists());
    }
}
