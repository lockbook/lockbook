use crate::local::LocalFileInfo;
use lb_rs::io::FsBaseEntry;
use lb_rs::model::file::File;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// An action to execute during sync.
#[derive(Debug)]
pub enum SyncAction {
    /// Pull a new or updated file from lockbook to local disk.
    PullToLocal { id: Uuid, local_path: String },
    /// Push a new local file to lockbook.
    PushToRemote { local_path: String, parent_id: Uuid, name: String },
    /// Update lockbook with locally modified content.
    UpdateRemote { id: Uuid, local_path: String },
    /// Delete a local file (remotely deleted).
    DeleteLocal { local_path: String },
    /// Delete a lockbook file (locally deleted).
    DeleteRemote { id: Uuid },
    /// Create a directory on local disk.
    CreateLocalDir { local_path: String },
    /// Create a folder in lockbook.
    CreateRemoteDir { local_path: String, parent_id: Uuid, name: String },
    /// Both sides changed — write remote version as conflict sidecar, keep local as-is.
    ConflictSidecar { id: Uuid, local_path: String },
}

/// Remote file info needed for reconciliation.
#[derive(Debug, Clone)]
pub struct RemoteFileInfo {
    pub id: Uuid,
    pub relative_path: String,
    pub is_folder: bool,
    pub last_modified: u64,
    pub parent: Uuid,
    pub name: String,
}

impl RemoteFileInfo {
    pub fn from_file(file: &File, relative_path: String) -> Self {
        Self {
            id: file.id,
            relative_path,
            is_folder: file.is_folder(),
            last_modified: file.last_modified,
            parent: file.parent,
            name: file.name.clone(),
        }
    }
}

/// Compute the list of sync actions given the three states.
///
/// `fs_base`: last agreed state (keyed by lockbook file UUID)
/// `local_tree`: current local disk state (keyed by relative path)
/// `remote_tree`: current lockbook state
pub fn reconcile(
    fs_base: &HashMap<Uuid, FsBaseEntry>,
    local_tree: &HashMap<String, LocalFileInfo>,
    remote_tree: &[RemoteFileInfo],
) -> Vec<SyncAction> {
    let mut actions = Vec::new();

    // Index remote files by path and by id
    let remote_by_path: HashMap<&str, &RemoteFileInfo> =
        remote_tree.iter().map(|r| (r.relative_path.as_str(), r)).collect();
    let remote_by_id: HashMap<Uuid, &RemoteFileInfo> =
        remote_tree.iter().map(|r| (r.id, r)).collect();

    // Index fs_base by path for reverse lookups
    let base_by_path: HashMap<&str, (Uuid, &FsBaseEntry)> =
        fs_base.iter().map(|(id, e)| (e.local_path.as_str(), (*id, e))).collect();

    // Track which paths and ids we've handled
    let mut handled_paths = HashSet::new();
    let mut handled_ids = HashSet::new();

    // 1. Process files known to fs_base (existing tracked files)
    for (id, base_entry) in fs_base {
        let path = &base_entry.local_path;
        let local = local_tree.get(path.as_str());
        let remote = remote_by_id.get(id);

        handled_paths.insert(path.as_str());
        handled_ids.insert(*id);

        match (local, remote) {
            // Both gone — nothing to do
            (None, None) => {}

            // Locally deleted, still remote — propagate delete
            (None, Some(_)) => {
                actions.push(SyncAction::DeleteRemote { id: *id });
            }

            // Remotely deleted, still local — propagate delete
            (Some(_), None) => {
                actions.push(SyncAction::DeleteLocal { local_path: path.clone() });
            }

            // Both exist — check for changes
            (Some(local_info), Some(remote_info)) => {
                if local_info.is_dir || remote_info.is_folder {
                    // Directories don't need content sync
                    continue;
                }

                let local_changed = local_info.content_hash != base_entry.content_hash;
                let remote_changed = remote_info.last_modified != base_entry.lb_last_modified;

                match (local_changed, remote_changed) {
                    (false, false) => {} // no-op
                    (true, false) => {
                        actions.push(SyncAction::UpdateRemote {
                            id: *id,
                            local_path: path.clone(),
                        });
                    }
                    (false, true) => {
                        actions.push(SyncAction::PullToLocal {
                            id: *id,
                            local_path: path.clone(),
                        });
                    }
                    (true, true) => {
                        actions.push(SyncAction::ConflictSidecar {
                            id: *id,
                            local_path: path.clone(),
                        });
                    }
                }
            }
        }
    }

    // 2. New remote files (not in fs_base)
    for remote in remote_tree {
        if handled_ids.contains(&remote.id) {
            continue;
        }

        let path = &remote.relative_path;

        if local_tree.contains_key(path.as_str()) {
            // Both sides created at the same path — treat as conflict
            if !remote.is_folder {
                actions.push(SyncAction::ConflictSidecar {
                    id: remote.id,
                    local_path: path.clone(),
                });
            }
        } else if remote.is_folder {
            actions.push(SyncAction::CreateLocalDir { local_path: path.clone() });
        } else {
            actions.push(SyncAction::PullToLocal {
                id: remote.id,
                local_path: path.clone(),
            });
        }

        handled_paths.insert(path.as_str());
    }

    // 3. New local files (not in fs_base, not matched to remote)
    for (path, local_info) in local_tree {
        if handled_paths.contains(path.as_str()) {
            continue;
        }
        if base_by_path.contains_key(path.as_str()) {
            continue;
        }

        // Find the parent in remote tree to get parent_id
        let parent_path = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let parent_id = if parent_path.is_empty() {
            // Will be resolved to root_id during apply
            Uuid::nil()
        } else {
            remote_by_path
                .get(parent_path.as_str())
                .map(|r| r.id)
                .unwrap_or(Uuid::nil())
        };

        let name = std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if local_info.is_dir {
            actions.push(SyncAction::CreateRemoteDir {
                local_path: path.clone(),
                parent_id,
                name,
            });
        } else {
            actions.push(SyncAction::PushToRemote {
                local_path: path.clone(),
                parent_id,
                name,
            });
        }
    }

    // Sort: create dirs before files (by depth), delete files before dirs (reverse depth)
    actions.sort_by(|a, b| action_sort_key(a).cmp(&action_sort_key(b)));
    actions
}

fn action_sort_key(action: &SyncAction) -> (u8, isize) {
    match action {
        // Dirs first, shallow to deep
        SyncAction::CreateLocalDir { local_path } | SyncAction::CreateRemoteDir { local_path, .. } => {
            (0, path_depth(local_path) as isize)
        }
        // Files next
        SyncAction::PullToLocal { local_path, .. }
        | SyncAction::PushToRemote { local_path, .. }
        | SyncAction::UpdateRemote { local_path, .. }
        | SyncAction::ConflictSidecar { local_path, .. } => {
            (1, path_depth(local_path) as isize)
        }
        // Deletes last, deep to shallow
        SyncAction::DeleteLocal { local_path } => (2, -(path_depth(local_path) as isize)),
        SyncAction::DeleteRemote { .. } => (2, 0),
    }
}

fn path_depth(path: &str) -> usize {
    path.chars().filter(|c| *c == '/').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(path: &str, hash: [u8; 32], modified: u64) -> FsBaseEntry {
        FsBaseEntry { local_path: path.to_string(), content_hash: hash, lb_last_modified: modified }
    }

    fn local(path: &str, hash: [u8; 32]) -> (String, LocalFileInfo) {
        (
            path.to_string(),
            LocalFileInfo { relative_path: path.to_string(), content_hash: hash, is_dir: false },
        )
    }

    fn remote(id: Uuid, path: &str, modified: u64) -> RemoteFileInfo {
        RemoteFileInfo {
            id,
            relative_path: path.to_string(),
            is_folder: false,
            last_modified: modified,
            parent: Uuid::nil(),
            name: path.to_string(),
        }
    }

    #[test]
    fn no_changes() {
        let id = Uuid::new_v4();
        let hash = [1u8; 32];
        let fs_base: HashMap<_, _> = [(id, base("a.txt", hash, 100))].into();
        let local: HashMap<_, _> = [local("a.txt", hash)].into();
        let remote = vec![remote(id, "a.txt", 100)];

        let actions = reconcile(&fs_base, &local, &remote);
        assert!(actions.is_empty());
    }

    #[test]
    fn local_edit_only() {
        let id = Uuid::new_v4();
        let old_hash = [1u8; 32];
        let new_hash = [2u8; 32];
        let fs_base: HashMap<_, _> = [(id, base("a.txt", old_hash, 100))].into();
        let local: HashMap<_, _> = [local("a.txt", new_hash)].into();
        let remote = vec![remote(id, "a.txt", 100)];

        let actions = reconcile(&fs_base, &local, &remote);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::UpdateRemote { id: aid, .. } if *aid == id));
    }

    #[test]
    fn remote_edit_only() {
        let id = Uuid::new_v4();
        let hash = [1u8; 32];
        let fs_base: HashMap<_, _> = [(id, base("a.txt", hash, 100))].into();
        let local: HashMap<_, _> = [local("a.txt", hash)].into();
        let remote = vec![remote(id, "a.txt", 200)]; // modified changed

        let actions = reconcile(&fs_base, &local, &remote);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::PullToLocal { id: aid, .. } if *aid == id));
    }

    #[test]
    fn both_changed_creates_conflict() {
        let id = Uuid::new_v4();
        let old_hash = [1u8; 32];
        let new_hash = [2u8; 32];
        let fs_base: HashMap<_, _> = [(id, base("a.txt", old_hash, 100))].into();
        let local: HashMap<_, _> = [local("a.txt", new_hash)].into();
        let remote = vec![remote(id, "a.txt", 200)];

        let actions = reconcile(&fs_base, &local, &remote);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::ConflictSidecar { .. }));
    }

    #[test]
    fn locally_deleted() {
        let id = Uuid::new_v4();
        let hash = [1u8; 32];
        let fs_base: HashMap<_, _> = [(id, base("a.txt", hash, 100))].into();
        let local: HashMap<_, _> = HashMap::new();
        let remote = vec![remote(id, "a.txt", 100)];

        let actions = reconcile(&fs_base, &local, &remote);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::DeleteRemote { .. }));
    }

    #[test]
    fn remotely_deleted() {
        let id = Uuid::new_v4();
        let hash = [1u8; 32];
        let fs_base: HashMap<_, _> = [(id, base("a.txt", hash, 100))].into();
        let local: HashMap<_, _> = [local("a.txt", hash)].into();
        let remote = vec![];

        let actions = reconcile(&fs_base, &local, &remote);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::DeleteLocal { .. }));
    }

    #[test]
    fn new_remote_file() {
        let id = Uuid::new_v4();
        let fs_base: HashMap<_, _> = HashMap::new();
        let local: HashMap<_, _> = HashMap::new();
        let remote = vec![remote(id, "new.txt", 100)];

        let actions = reconcile(&fs_base, &local, &remote);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::PullToLocal { .. }));
    }

    #[test]
    fn new_local_file() {
        let fs_base: HashMap<_, _> = HashMap::new();
        let local: HashMap<_, _> = [local("new.txt", [3u8; 32])].into();
        let remote = vec![];

        let actions = reconcile(&fs_base, &local, &remote);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], SyncAction::PushToRemote { .. }));
    }
}
