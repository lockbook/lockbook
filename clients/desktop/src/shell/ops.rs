//! Shared session mutations used from `apply` (cache rebuild, pins).

use lb::Uuid;
use workspace_rs::file_cache::FileCache;

use super::session::Ready;

pub fn rebuild_cache(r: &mut Ready) {
    if let Ok(files) = FileCache::new(&r.workspace.core) {
        *r.workspace.files.write().unwrap() = files;
    }
    r.files_epoch = r.files_epoch.wrapping_add(1);
    refresh_pinned(r);
    r.refresh_status();
}

pub fn refresh_pinned(r: &mut Ready) {
    r.pinned = r.workspace.core.list_pinned().unwrap_or_default();
}

pub fn is_pinned(r: &Ready, id: Uuid) -> bool {
    r.pinned.contains(&id)
}
