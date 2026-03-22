pub mod error;
pub mod hash;
pub mod ignore;
pub mod local;
pub mod reconcile;
pub mod watcher;

use crate::error::SyncDirError;
use crate::ignore::IgnoreRules;
use crate::local::{
    delete_local, scan_local_tree, write_conflict_sidecar, write_local_file,
};
use crate::reconcile::{reconcile, RemoteFileInfo, SyncAction};
use crate::watcher::FsWatcher;
use lb_rs::io::FsBaseEntry;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use lb_rs::Lb;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

pub struct SyncDirConfig {
    /// Lockbook folder path (e.g. ".openclaw").
    pub lockbook_folder: String,
    /// Local directory to sync (e.g. "/home/user/.openclaw").
    pub local_dir: PathBuf,
    /// Remote polling interval (default: 5s).
    pub pull_interval: Duration,
    /// Whether to use a filesystem watcher (default: true).
    pub watch: bool,
    /// Run one reconciliation cycle and exit.
    pub once: bool,
}

impl Default for SyncDirConfig {
    fn default() -> Self {
        Self {
            lockbook_folder: String::new(),
            local_dir: PathBuf::new(),
            pull_interval: Duration::from_secs(5),
            watch: true,
            once: false,
        }
    }
}

/// Run a single reconciliation cycle and exit.
pub async fn run_once(lb: &Lb, config: &SyncDirConfig) -> Result<(), SyncDirError> {
    let root_id = resolve_or_create_lb_folder(lb, &config.lockbook_folder).await?;

    fs::create_dir_all(&config.local_dir)
        .map_err(|e| SyncDirError::LocalDirCreateFailed(config.local_dir.clone(), e))?;

    IgnoreRules::generate_default_file(&config.local_dir)?;
    let ignore_rules = IgnoreRules::load(&config.local_dir);

    // Pull latest from server
    tracing::info!("syncing with lockbook server");
    lb.sync().await.map_err(|e| SyncDirError::Lb(e.kind))?;

    run_cycle(lb, config, &ignore_rules, root_id).await?;

    // Push changes back to server
    tracing::info!("pushing changes to lockbook server");
    lb.sync().await.map_err(|e| SyncDirError::Lb(e.kind))?;

    Ok(())
}

/// Run the long-lived sync loop with filesystem watching and periodic remote polling.
pub async fn run(lb: &Lb, config: &SyncDirConfig) -> Result<(), SyncDirError> {
    let root_id = resolve_or_create_lb_folder(lb, &config.lockbook_folder).await?;

    fs::create_dir_all(&config.local_dir)
        .map_err(|e| SyncDirError::LocalDirCreateFailed(config.local_dir.clone(), e))?;

    IgnoreRules::generate_default_file(&config.local_dir)?;
    let ignore_rules = IgnoreRules::load(&config.local_dir);

    // Initial sync
    tracing::info!("initial sync with lockbook server");
    lb.sync().await.map_err(|e| SyncDirError::Lb(e.kind))?;
    run_cycle(lb, config, &ignore_rules, root_id).await?;
    lb.sync().await.map_err(|e| SyncDirError::Lb(e.kind))?;

    let mut watcher = if config.watch {
        Some(FsWatcher::new(&config.local_dir, &ignore_rules)?)
    } else {
        None
    };

    let mut pull_interval = tokio::time::interval(config.pull_interval);
    // Skip the first tick (we just synced)
    pull_interval.tick().await;

    tracing::info!(
        "sync-dir running: {} <-> {}",
        config.lockbook_folder,
        config.local_dir.display()
    );

    loop {
        tokio::select! {
            Some(paths) = async {
                match watcher.as_mut() {
                    Some(w) => w.next_batch().await,
                    None => std::future::pending().await,
                }
            } => {
                tracing::debug!("local changes detected: {} paths", paths.len());
                if let Err(e) = run_cycle(lb, config, &ignore_rules, root_id).await {
                    tracing::error!("sync cycle failed: {e}");
                }
                if let Err(e) = lb.sync().await {
                    tracing::error!("lockbook sync failed: {e:?}");
                }
            }
            _ = pull_interval.tick() => {
                if let Err(e) = lb.sync().await {
                    tracing::error!("lockbook sync failed: {e:?}");
                }
                if let Err(e) = run_cycle(lb, config, &ignore_rules, root_id).await {
                    tracing::error!("sync cycle failed: {e}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutting down sync-dir");
                break;
            }
        }
    }

    Ok(())
}

async fn run_cycle(
    lb: &Lb,
    config: &SyncDirConfig,
    ignore: &IgnoreRules,
    root_id: Uuid,
) -> Result<(), SyncDirError> {
    // Scan local tree
    let local_tree = scan_local_tree(&config.local_dir, ignore)?;

    // Scan remote tree
    let remote_files = lb
        .get_and_get_children_recursively(&root_id)
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?;

    let remote_tree = build_remote_tree(&remote_files, root_id);

    // Load fs_base
    let fs_base: HashMap<Uuid, FsBaseEntry> = lb
        .get_fs_base()
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?
        .into_iter()
        .collect();

    // Reconcile
    let actions = reconcile(&fs_base, &local_tree, &remote_tree);

    if actions.is_empty() {
        tracing::debug!("no changes to sync");
        return Ok(());
    }

    tracing::info!("{} sync actions to apply", actions.len());

    // Apply actions
    apply_actions(lb, &config.local_dir, root_id, &actions).await?;

    // Rebuild and save fs_base
    let new_base = build_new_fs_base(lb, &config.local_dir, ignore, root_id).await?;
    lb.set_fs_base(new_base)
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?;

    Ok(())
}

/// Build relative paths for remote files relative to the sync root.
fn build_remote_tree(files: &[File], root_id: Uuid) -> Vec<RemoteFileInfo> {
    // Build a parent->children map and id->file map
    let by_id: HashMap<Uuid, &File> = files.iter().map(|f| (f.id, f)).collect();

    let mut result = Vec::new();
    for file in files {
        if file.id == root_id {
            continue; // skip the root folder itself
        }
        if let Some(rel_path) = compute_relative_path(file, root_id, &by_id) {
            result.push(RemoteFileInfo::from_file(file, rel_path));
        }
    }
    result
}

/// Walk parent chain to build a relative path from root_id.
fn compute_relative_path(
    file: &File,
    root_id: Uuid,
    by_id: &HashMap<Uuid, &File>,
) -> Option<String> {
    let mut parts = vec![file.name.clone()];
    let mut current = file.parent;

    loop {
        if current == root_id {
            parts.reverse();
            return Some(parts.join("/"));
        }
        let parent = by_id.get(&current)?;
        parts.push(parent.name.clone());
        if parent.parent == current {
            // Reached the absolute root without finding our sync root
            return None;
        }
        current = parent.parent;
    }
}

async fn apply_actions(
    lb: &Lb,
    local_dir: &Path,
    root_id: Uuid,
    actions: &[SyncAction],
) -> Result<(), SyncDirError> {
    for action in actions {
        match action {
            SyncAction::CreateLocalDir { local_path } => {
                let full = local_dir.join(local_path);
                tracing::info!("creating local dir: {local_path}");
                fs::create_dir_all(&full)?;
            }

            SyncAction::PullToLocal { id, local_path } => {
                tracing::info!("pulling: {local_path}");
                let content = lb
                    .read_document(*id, false)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
                write_local_file(local_dir, local_path, &content)?;
            }

            SyncAction::PushToRemote { local_path, parent_id, name } => {
                tracing::info!("pushing: {local_path}");
                let parent = if *parent_id == Uuid::nil() { root_id } else { *parent_id };
                let content = fs::read(local_dir.join(local_path))?;
                let file = lb
                    .create_file(name, &parent, FileType::Document)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
                lb.write_document(file.id, &content)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
            }

            SyncAction::CreateRemoteDir { local_path: _, parent_id, name } => {
                tracing::info!("creating remote dir: {name}");
                let parent = if *parent_id == Uuid::nil() { root_id } else { *parent_id };
                lb.create_file(name, &parent, FileType::Folder)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
            }

            SyncAction::UpdateRemote { id, local_path } => {
                tracing::info!("updating remote: {local_path}");
                let content = fs::read(local_dir.join(local_path))?;
                lb.write_document(*id, &content)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
            }

            SyncAction::DeleteLocal { local_path } => {
                tracing::info!("deleting local: {local_path}");
                delete_local(local_dir, local_path)?;
            }

            SyncAction::DeleteRemote { id } => {
                tracing::info!("deleting remote: {id}");
                lb.delete(id).await.map_err(|e| SyncDirError::Lb(e.kind))?;
            }

            SyncAction::ConflictSidecar { id, local_path } => {
                tracing::info!("conflict: {local_path} — creating sidecar");
                // Write the remote version as a sidecar
                let remote_content = lb
                    .read_document(*id, false)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
                write_conflict_sidecar(local_dir, local_path, &remote_content)?;
                // Keep local version as-is, push it to remote
                let local_content = fs::read(local_dir.join(local_path))?;
                lb.write_document(*id, &local_content)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
            }
        }
    }
    Ok(())
}

/// Build a fresh fs_base snapshot from current local + remote state.
async fn build_new_fs_base(
    lb: &Lb,
    local_dir: &Path,
    ignore: &IgnoreRules,
    root_id: Uuid,
) -> Result<Vec<(Uuid, FsBaseEntry)>, SyncDirError> {
    let remote_files = lb
        .get_and_get_children_recursively(&root_id)
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?;

    let by_id: HashMap<Uuid, &File> = remote_files.iter().map(|f| (f.id, f)).collect();
    let mut entries = Vec::new();

    for file in &remote_files {
        if file.id == root_id || file.is_folder() {
            continue;
        }

        if let Some(rel_path) = compute_relative_path(file, root_id, &by_id) {
            let local_path = local_dir.join(&rel_path);

            if ignore.is_ignored(&local_path, false) {
                continue;
            }

            let content_hash = if local_path.exists() {
                crate::hash::hash_file(&local_path).unwrap_or([0; 32])
            } else {
                [0; 32]
            };

            entries.push((
                file.id,
                FsBaseEntry {
                    local_path: rel_path,
                    content_hash,
                    lb_last_modified: file.last_modified,
                },
            ));
        }
    }

    Ok(entries)
}

/// Resolve a lockbook folder by path, creating it if it doesn't exist.
async fn resolve_or_create_lb_folder(
    lb: &Lb,
    folder_path: &str,
) -> Result<Uuid, SyncDirError> {
    match lb.get_by_path(folder_path).await {
        Ok(file) => Ok(file.id),
        Err(_) => {
            tracing::info!("creating lockbook folder: {folder_path}");
            let file = lb
                .create_at_path(&format!("{folder_path}/"))
                .await
                .map_err(|e| SyncDirError::Lb(e.kind))?;
            Ok(file.id)
        }
    }
}
