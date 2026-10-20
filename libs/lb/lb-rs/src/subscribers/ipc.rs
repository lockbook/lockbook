use std::fs::{self, DirBuilder, File};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, OpenOptionsExt};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

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
        let socket = UnixDatagram::unbound()?;
        socket.set_nonblocking(true)?;
        let (path, socket) = if listen {
            // Serialize allocation and stale-socket removal so a reused name cannot
            // be unlinked by another process acting on an earlier failed send.
            let _lock = Self::lock_directory(&directory)?;
            let mut index = 0u64;
            loop {
                let path = directory.join(index.to_string());
                match UnixDatagram::bind(&path) {
                    Ok(bound) => break (Some(path), bound),
                    Err(error) if error.kind() == ErrorKind::AddrInUse => {
                        let metadata = match fs::symlink_metadata(&path) {
                            Ok(metadata) => metadata,
                            Err(error) if error.kind() == ErrorKind::NotFound => continue,
                            Err(error) => return Err(error),
                        };
                        if metadata.file_type().is_socket()
                            && socket
                                .send_to(&[1], &path)
                                .is_err_and(|error| error.kind() == ErrorKind::ConnectionRefused)
                        {
                            fs::remove_file(&path)?;
                            continue;
                        }
                        index = index
                            .checked_add(1)
                            .ok_or_else(|| io::Error::other("no available IPC socket names"))?;
                    }
                    Err(error) => return Err(error),
                }
            }
        } else {
            (None, socket)
        };
        let ipc = Self { directory, path, socket };
        ipc.socket.set_nonblocking(true)?;
        Ok(ipc)
    }

    fn lock_directory(directory: &std::path::Path) -> io::Result<File> {
        // Never unlink this file: every process must lock the same inode.
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(directory.join("lock"))?;
        lock.lock()?;
        Ok(lock)
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
            match self.socket.send_to(&[1], &path) {
                Ok(_) => {}
                // A queued wake already tells a slow peer to read everything it has missed.
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) if error.kind() == ErrorKind::ConnectionRefused => {
                    let _lock = Self::lock_directory(&self.directory)?;
                    // Allocation may have reused this name since the first send.
                    if self
                        .socket
                        .send_to(&[1], &path)
                        .is_err_and(|error| error.kind() == ErrorKind::ConnectionRefused)
                    {
                        let _ = fs::remove_file(&path);
                    }
                }
                Err(error) => warn!(?error, ?path, "could not wake database peer"),
            }
        }
        Ok(())
    }
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

    #[test]
    fn reuses_dead_and_dropped_slots_without_displacing_live_peers() {
        let directory = tempdir_in("/tmp").unwrap();
        let db = CoreDb::init(&Config::default().log_location(directory.path())).unwrap();
        let first = Ipc::new(&db, true).unwrap();
        assert_eq!(first.path.as_ref().unwrap().file_name().unwrap(), "0");
        let stale = first.directory.join("1");
        drop(UnixDatagram::bind(&stale).unwrap());
        let second = Ipc::new(&db, true).unwrap();
        assert_eq!(second.path.as_ref(), Some(&stale));
        // Drain the liveness probe sent while allocating the second socket.
        assert_eq!(first.socket.recv(&mut [0]).unwrap(), 1);
        second.send().unwrap();
        assert_eq!(first.socket.recv(&mut [0]).unwrap(), 1);
        let path = first.path.clone();
        drop(first);
        let replacement = Ipc::new(&db, true).unwrap();
        assert_eq!(replacement.path, path);
        replacement.send().unwrap();
        assert_eq!(second.socket.recv(&mut [0]).unwrap(), 1);
    }

    #[test]
    fn concurrent_allocations_keep_every_peer_reachable() {
        let directory = tempdir_in("/tmp").unwrap();
        let db = CoreDb::init(&Config::default().log_location(directory.path())).unwrap();
        let sender = Ipc::new(&db, false).unwrap();
        // Make all allocators contend for the same dead socket.
        drop(UnixDatagram::bind(sender.directory.join("0")).unwrap());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let path = directory.path().to_path_buf();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let db = CoreDb::init(&Config::default().log_location(path)).unwrap();
                    barrier.wait();
                    Ipc::new(&db, true).unwrap()
                })
            })
            .collect();
        let peers: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        for peer in &peers {
            while peer.socket.recv(&mut [0]).is_ok() {}
        }
        sender.send().unwrap();
        for peer in &peers {
            assert_eq!(peer.socket.recv(&mut [0]).unwrap(), 1);
        }
    }

    #[test]
    fn starts_with_a_database_path_too_long_for_uuid_socket_names() {
        let directory = tempdir_in("/tmp").unwrap();
        let path = directory.path().join("a".repeat(45));
        fs::create_dir(&path).unwrap();
        let db = CoreDb::init(&Config::default().log_location(path)).unwrap();
        let ipc = Ipc::new(&db, true).unwrap();
        assert!(
            ipc.directory
                .join(format!("{}.sock", "a".repeat(32)))
                .as_os_str()
                .len()
                > 103
        );
        assert!(ipc.path.as_ref().unwrap().exists());
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
