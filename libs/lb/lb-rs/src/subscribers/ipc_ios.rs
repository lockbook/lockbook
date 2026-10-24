use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, RawFd};

use db_rs::View;
use sha2::{Digest, Sha256};
use tokio::io::unix::AsyncFd;
use tokio::runtime::Handle;
use tokio::task;

use crate::io::CoreDb;
use crate::model::errors::Unexpected;
use crate::{Lb, LbResult};

pub(crate) struct Ipc {
    name: CString,
}

impl Ipc {
    pub(crate) fn new(db: &CoreDb, _listen: bool) -> io::Result<Self> {
        let directory = db.log().directory.canonicalize()?;
        let hash = Sha256::digest(directory.as_os_str().as_encoded_bytes());
        Ok(Self { name: CString::new(format!("app.lockbook.database.{hash:x}")).unwrap() })
    }

    pub(crate) fn notify_peers(&self) {
        unsafe {
            notify_post(self.name.as_ptr());
        }
    }
}

struct Notification {
    token: i32,
    fd: RawFd,
}

impl AsRawFd for Notification {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}
impl Drop for Notification {
    fn drop(&mut self) {
        unsafe {
            // notify_cancel also closes the descriptor.
            notify_cancel(self.token);
        }
    }
}

impl Lb {
    pub fn change_notification_name(&self) -> &str {
        self.ipc.name.to_str().unwrap()
    }

    pub(crate) async fn setup_ipc(&self) -> LbResult<()> {
        if !self.config.background_work {
            return Ok(());
        }
        let mut fd = -1;
        let mut token = 0;
        let status = unsafe {
            notify_register_file_descriptor(self.ipc.name.as_ptr(), &mut fd, 0, &mut token)
        };
        if status != 0 {
            return Err(
                io::Error::other(format!("notification registration failed: {status}")).into()
            );
        }
        let fd = Notification { token, fd };
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags == -1
            || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
        {
            return Err(io::Error::last_os_error().into());
        }
        let fd = AsyncFd::new(fd)?;
        self.catch_up_from_ipc().await?;
        let core = self.clone();
        tokio::spawn(async move {
            loop {
                let result = async {
                    let mut ready = fd.readable().await?;
                    let mut bytes = [0u8; 256];
                    let read = ready.try_io(|fd| {
                        let count = unsafe {
                            libc::read(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len())
                        };
                        if count < 0 {
                            Err(io::Error::last_os_error())
                        } else if count == 0 {
                            Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "notification pipe closed",
                            ))
                        } else {
                            Ok(())
                        }
                    });
                    match read {
                        Ok(result) => result,
                        Err(_) => Ok(()),
                    }
                }
                .await;
                if let Err(error) = result {
                    error!(?error, "database notification failed");
                    break;
                }
                if let Err(error) = core.catch_up_from_ipc().await {
                    error!(?error, "database catch-up failed");
                    break;
                }
            }
        });
        Ok(())
    }

    async fn catch_up_from_ipc(&self) -> LbResult<()> {
        let core = self.clone();
        let runtime = Handle::current();
        task::spawn_blocking(move || runtime.block_on(async { core.begin_tx().await.end() }))
            .await
            .map_unexpected()
    }
}

unsafe extern "C" {
    fn notify_post(name: *const std::ffi::c_char) -> u32;
    fn notify_register_file_descriptor(
        name: *const std::ffi::c_char, fd: *mut i32, flags: i32, token: *mut i32,
    ) -> u32;
    fn notify_cancel(token: i32) -> u32;
}
