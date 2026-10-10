use std::fs::{self, DirBuilder};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;

use db_rs::View;
use db_rs::log::Notification;
use tokio::net::UnixDatagram as AsyncUnixDatagram;
use tokio::runtime::Handle;
use tokio::task;
use uuid::Uuid;

use crate::io::CoreDb;
use crate::model::errors::Unexpected;
use crate::{Lb, LbResult};

pub(crate) struct Ipc {
    directory: PathBuf,
    path: Option<PathBuf>,
    socket: UnixDatagram,
    notifications: Mutex<Receiver<Notification>>,
}

impl Ipc {
    pub(crate) fn new(db: &mut CoreDb, listen: bool) -> io::Result<Self> {
        let directory = db.log().directory.canonicalize()?.join("ipc");
        DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&directory)?;
        let path = listen.then(|| directory.join(format!("{}.sock", Uuid::new_v4().simple())));
        let socket = match &path {
            Some(path) => UnixDatagram::bind(path)?,
            None => UnixDatagram::unbound()?,
        };
        let ipc = Self {
            directory,
            path,
            socket,
            notifications: Mutex::new(db.log_mut().notifications()),
        };
        ipc.socket.set_nonblocking(true)?;
        Ok(ipc)
    }

    pub(crate) fn notify_peers(&self) {
        // Consume catch-up notifications too, but only broadcast our own commits.
        let mut changed = false;
        for notification in self.notifications.lock().unwrap().try_iter() {
            changed |= notification.local;
        }
        if !changed {
            return;
        }
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
                    // Socket names are unique per instance, so a dead peer's name is never reused.
                    let _ = fs::remove_file(&path);
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
    fn sends_to_peers_skips_self_and_cleans_up_dead_sockets() {
        let directory = tempdir_in("/tmp").unwrap();
        let mut db = CoreDb::init(&Config::default().log_location(directory.path())).unwrap();
        let ipc = Ipc::new(&mut db, true).unwrap();
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

    #[test]
    fn only_local_commits_send_notifications() {
        let directory = tempdir_in("/tmp").unwrap();
        let config = Config::default().log_location(directory.path());
        let mut db = CoreDb::init(&config).unwrap();
        let ipc = Ipc::new(&mut db, false).unwrap();
        let peer = UnixDatagram::bind(ipc.directory.join("peer.sock")).unwrap();
        peer.set_nonblocking(true).unwrap();

        db.write_tx().unwrap().end_tx(&mut db).unwrap();
        ipc.notify_peers();
        assert_eq!(peer.recv(&mut [0]).unwrap_err().kind(), ErrorKind::WouldBlock);

        let tx = db.write_tx().unwrap();
        db.schema.last_synced.replace(123).unwrap();
        tx.end_tx(&mut db).unwrap();
        ipc.notify_peers();
        assert_eq!(peer.recv(&mut [0]).unwrap(), 1);

        let mut other = CoreDb::init(&config).unwrap();
        let tx = other.write_tx().unwrap();
        other.schema.last_synced.replace(456).unwrap();
        tx.end_tx(&mut other).unwrap();
        db.write_tx().unwrap().end_tx(&mut db).unwrap();
        ipc.notify_peers();
        assert_eq!(db.schema.last_synced.as_ref(), Some(&456));
        assert_eq!(peer.recv(&mut [0]).unwrap_err().kind(), ErrorKind::WouldBlock);
    }

    #[test]
    fn a_slow_peer_does_not_block_notifications_to_other_peers() {
        let directory = tempdir_in("/tmp").unwrap();
        let mut db = CoreDb::init(&Config::default().log_location(directory.path())).unwrap();
        let ipc = Ipc::new(&mut db, false).unwrap();
        let slow_path = ipc.directory.join("slow.sock");
        let _slow = UnixDatagram::bind(&slow_path).unwrap();
        loop {
            if let Err(error) = ipc.socket.send_to(&[1], &slow_path) {
                assert!(
                    error.kind() == ErrorKind::WouldBlock
                        || error.raw_os_error() == Some(libc::ENOBUFS),
                    "{error:?}"
                );
                break;
            }
        }
        let peer = UnixDatagram::bind(ipc.directory.join("peer.sock")).unwrap();
        peer.set_nonblocking(true).unwrap();

        ipc.send().unwrap();

        assert_eq!(peer.recv(&mut [0]).unwrap(), 1);
    }
}
