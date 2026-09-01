//! Shared session mutations used from `apply` (cache epoch, pins).

use lb::Uuid;
use lb::model::file::File;
use workspace_rs::file_cache::FilesExt;

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

/// File in your tree that you don't own. `list_metadatas` hides the Link and
/// shows the target (e.g. luca's `movies.md` sitting in your root), so
/// `FileType::Link` never appears in the UI. Removing it deletes your link,
/// not the owner's file.
pub fn is_saved_share(f: &File, me: &str) -> bool {
    !f.owner.eq_ignore_ascii_case(me)
}

pub fn ids_are_saved_shares(files: &impl FilesExt, ids: &[Uuid], me: &str) -> bool {
    !ids.is_empty()
        && ids
            .iter()
            .all(|&id| files.get_by_id(id).is_some_and(|f| is_saved_share(f, me)))
}
