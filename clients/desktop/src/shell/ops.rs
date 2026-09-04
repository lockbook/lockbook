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

/// Unowned file whose tree parent is owned by `me` (accepted link after parent rewrite).
pub fn is_saved_share(f: &File, parent: Option<&File>, me: &str) -> bool {
    !f.owner.eq_ignore_ascii_case(me) && parent.is_some_and(|p| p.owner.eq_ignore_ascii_case(me))
}

pub fn ids_are_saved_shares(files: &impl FilesExt, ids: &[Uuid], me: &str) -> bool {
    !ids.is_empty()
        && ids.iter().all(|&id| {
            files
                .get_by_id(id)
                .is_some_and(|f| is_saved_share(f, files.get_by_id(f.parent), me))
        })
}

#[cfg(test)]
mod tests {
    use super::is_saved_share;
    use lb::Uuid;
    use lb::model::file::File;
    use lb::model::file_metadata::FileType;

    fn file(id: Uuid, parent: Uuid, owner: &str) -> File {
        File {
            id,
            parent,
            name: "x".into(),
            file_type: FileType::Folder,
            last_modified: 0,
            last_modified_by: owner.into(),
            owner: owner.into(),
            shares: Vec::new(),
            size_bytes: 0,
        }
    }

    #[test]
    fn saved_share_is_unowned_file_parented_to_me() {
        let me_root = Uuid::new_v4();
        let shared = Uuid::new_v4();
        let mine = file(me_root, me_root, "jane");
        let root_share = file(shared, me_root, "luca");
        let child = file(Uuid::new_v4(), shared, "luca");
        // Nested folder also shared with jane, but she did not accept a second link —
        // parent is still luca's shared folder, not jane's.
        let nested_also_shared = file(Uuid::new_v4(), shared, "luca");

        assert!(is_saved_share(&root_share, Some(&mine), "jane"));
        assert!(!is_saved_share(&child, Some(&root_share), "jane"));
        assert!(!is_saved_share(&nested_also_shared, Some(&root_share), "jane"));
        assert!(!is_saved_share(&mine, Some(&mine), "jane"));
    }

    #[test]
    fn second_link_to_nested_folder_is_its_own_saved_share() {
        let me_root = Uuid::new_v4();
        let mine = file(me_root, me_root, "jane");
        // Closest-link rewrite: nested folder's parent is jane's root, not luca's outer folder.
        let nested = file(Uuid::new_v4(), me_root, "luca");
        assert!(is_saved_share(&nested, Some(&mine), "jane"));
    }
}
