//! Shared session mutations used from `apply` (cache epoch, pins).

use lb::Uuid;

use super::session::Ready;

/// Invalidate Recents/Shared after workspace already wrote `files`.
#[tracing::instrument(level = "trace", skip_all)]
pub fn note_files_changed(r: &mut Ready) {
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
