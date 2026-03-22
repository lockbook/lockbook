use std::path::PathBuf;

#[derive(Debug)]
pub enum SyncDirError {
    Lb(lb_rs::LbErrKind),
    Io(std::io::Error),
    WatcherInit(notify::Error),
    LockbookFolderNotFound(String),
    LocalDirCreateFailed(PathBuf, std::io::Error),
}

impl std::fmt::Display for SyncDirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lb(e) => write!(f, "lockbook error: {e:?}"),
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::WatcherInit(e) => write!(f, "filesystem watcher error: {e}"),
            Self::LockbookFolderNotFound(p) => write!(f, "lockbook folder not found: {p}"),
            Self::LocalDirCreateFailed(p, e) => {
                write!(f, "failed to create local dir {}: {e}", p.display())
            }
        }
    }
}

impl std::error::Error for SyncDirError {}

impl From<lb_rs::LbErrKind> for SyncDirError {
    fn from(e: lb_rs::LbErrKind) -> Self {
        Self::Lb(e)
    }
}

impl From<std::io::Error> for SyncDirError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<notify::Error> for SyncDirError {
    fn from(e: notify::Error) -> Self {
        Self::WatcherInit(e)
    }
}
