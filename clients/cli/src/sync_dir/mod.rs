mod ignore;
mod local;
mod watcher;

use cli_rs::cli_error::{CliError, CliResult};
use ignore::IgnoreRules;
use lb_rs::model::core_config::Config;
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::File;
use lb_rs::model::ValidationFailure;
use lb_rs::Lb;
use local::{
    delete_local, hash_bytes, hash_file, scan_local_tree, write_conflict_sidecar,
    write_local_file, LocalFileInfo,
};
use watcher::FsWatcher;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

use crate::ensure_account;

// --- CLI entry point ---

#[tokio::main]
pub async fn run(
    lockbook_folder: String,
    local_dir: String,
    pull_interval: Option<String>,
    no_watch: bool,
    once: bool,
) -> CliResult<()> {
    let lb = Lb::init(Config::cli_config("cli"))
        .await
        .map_err(|err| CliError::from(err.to_string()))?;
    ensure_account(&lb)?;

    let pull_interval = match pull_interval {
        Some(s) => parse_duration(&s)?,
        None => Duration::from_secs(5),
    };

    let local_dir = PathBuf::from(local_dir);

    if once {
        sync_once(&lb, &lockbook_folder, &local_dir, pull_interval)
            .await
            .map_err(|e| CliError::from(e.to_string()))?;
    } else {
        sync_loop(&lb, &lockbook_folder, &local_dir, pull_interval, !no_watch)
            .await
            .map_err(|e| CliError::from(e.to_string()))?;
    }

    Ok(())
}

fn parse_duration(s: &str) -> CliResult<Duration> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|_| CliError::from(format!("invalid duration: {s}")))
    } else if let Some(ms) = s.strip_suffix("ms") {
        ms.parse::<u64>()
            .map(Duration::from_millis)
            .map_err(|_| CliError::from(format!("invalid duration: {s}")))
    } else if let Some(m) = s.strip_suffix('m') {
        m.parse::<u64>()
            .map(|v| Duration::from_secs(v * 60))
            .map_err(|_| CliError::from(format!("invalid duration: {s}")))
    } else {
        s.parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|_| {
                CliError::from(format!("invalid duration: {s} (expected e.g. 5s, 500ms, 1m)"))
            })
    }
}

// --- Sync error ---

#[derive(Debug)]
enum SyncDirError {
    Lb(LbErrKind),
    Io(std::io::Error),
    WatcherInit(notify::Error),
    LocalDirCreateFailed(PathBuf, std::io::Error),
}

impl std::fmt::Display for SyncDirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lb(e) => write!(f, "lockbook error: {e:?}"),
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::WatcherInit(e) => write!(f, "filesystem watcher error: {e}"),
            Self::LocalDirCreateFailed(p, e) => {
                write!(f, "failed to create local dir {}: {e}", p.display())
            }
        }
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

// --- fs_base: last agreed state between local filesystem and lockbook ---

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FsBaseEntry {
    local_path: String,
    content_hash: [u8; 32],
    lb_last_modified: u64,
}

fn load_fs_base(local_dir: &Path) -> HashMap<Uuid, FsBaseEntry> {
    let path = local_dir.join(".sync-dir-state");
    match fs::read(&path) {
        Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_fs_base(local_dir: &Path, entries: &[(Uuid, FsBaseEntry)]) -> std::io::Result<()> {
    let map: HashMap<&Uuid, &FsBaseEntry> = entries.iter().map(|(id, e)| (id, e)).collect();
    let data = serde_json::to_vec(&map).expect("serialize fs_base");
    let path = local_dir.join(".sync-dir-state");
    let tmp = tempfile::NamedTempFile::new_in(local_dir)?;
    fs::write(tmp.path(), &data)?;
    tmp.persist(&path).map_err(|e| e.error)?;
    Ok(())
}

// --- Local change detection ---

#[derive(Debug)]
enum LocalChange {
    NewFile { path: String, is_dir: bool },
    Modified { path: String, id: Uuid },
    Deleted { id: Uuid },
}

// --- Orchestration ---

/// Run a single reconciliation cycle and exit.
async fn sync_once(
    lb: &Lb,
    lockbook_folder: &str,
    local_dir: &Path,
    _pull_interval: Duration,
) -> Result<(), SyncDirError> {
    let root_path = resolve_or_create_lb_folder(lb, lockbook_folder).await?;

    fs::create_dir_all(local_dir)
        .map_err(|e| SyncDirError::LocalDirCreateFailed(local_dir.to_path_buf(), e))?;

    IgnoreRules::generate_default_file(local_dir)?;
    let ignore_rules = IgnoreRules::load(local_dir);

    run_cycle(lb, local_dir, &ignore_rules, &root_path).await?;

    Ok(())
}

/// Run the long-lived sync loop with filesystem watching and periodic remote polling.
async fn sync_loop(
    lb: &Lb,
    lockbook_folder: &str,
    local_dir: &Path,
    pull_interval: Duration,
    watch: bool,
) -> Result<(), SyncDirError> {
    let root_path = resolve_or_create_lb_folder(lb, lockbook_folder).await?;

    fs::create_dir_all(local_dir)
        .map_err(|e| SyncDirError::LocalDirCreateFailed(local_dir.to_path_buf(), e))?;

    IgnoreRules::generate_default_file(local_dir)?;
    let ignore_rules = IgnoreRules::load(local_dir);

    // Initial cycle
    run_cycle(lb, local_dir, &ignore_rules, &root_path).await?;

    let mut watcher = if watch {
        Some(FsWatcher::new(local_dir, &ignore_rules)?)
    } else {
        None
    };

    let mut interval = tokio::time::interval(pull_interval);
    interval.tick().await; // skip first tick (we just synced)

    tracing::info!(
        "sync-dir running: {} <-> {}",
        lockbook_folder,
        local_dir.display()
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
                if let Err(e) = run_cycle(lb, local_dir, &ignore_rules, &root_path).await {
                    tracing::error!("sync cycle failed: {e}");
                }
            }
            _ = interval.tick() => {
                if let Err(e) = run_cycle(lb, local_dir, &ignore_rules, &root_path).await {
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

/// One full sync cycle following the RFC's 6-step pattern:
/// 1. Diff filesystem against fs_base → local changes
/// 2. (implicit) lb-rs changes detected after sync
/// 3. Apply local changes into lb-rs
/// 4. lb.sync() → core handles server merge/conflicts
/// 5. Materialize lb-rs state to disk
/// 6. Advance fs_base
async fn run_cycle(
    lb: &Lb,
    local_dir: &Path,
    ignore: &IgnoreRules,
    root_path: &str,
) -> Result<(), SyncDirError> {
    // Step 1: detect local changes
    let local_tree = scan_local_tree(local_dir, ignore)?;
    let fs_base = load_fs_base(local_dir);
    let changes = detect_local_changes(&local_tree, &fs_base);

    // Step 3: apply local changes to lb-rs
    if !changes.is_empty() {
        tracing::info!("{} local changes to apply", changes.len());
        apply_local_to_lb(lb, root_path, &changes, local_dir).await?;
    }

    // Step 4: sync with server
    tracing::debug!("syncing with lockbook server");
    lb.sync().await.map_err(|e| SyncDirError::Lb(e.kind))?;

    // Step 5: materialize resolved state to disk
    let remote_tree = get_remote_tree(lb, root_path).await?;
    let local_tree = scan_local_tree(local_dir, ignore)?;
    materialize_to_disk(lb, local_dir, ignore, &remote_tree, &local_tree, &fs_base).await?;

    // Step 6: advance fs_base
    let new_base = build_new_fs_base(local_dir, ignore, &remote_tree)?;
    save_fs_base(local_dir, &new_base)?;

    Ok(())
}

// --- Step 1: detect local changes ---

fn detect_local_changes(
    local_tree: &HashMap<String, LocalFileInfo>,
    fs_base: &HashMap<Uuid, FsBaseEntry>,
) -> Vec<LocalChange> {
    let mut changes = Vec::new();

    let base_by_path: HashMap<&str, (Uuid, &FsBaseEntry)> =
        fs_base.iter().map(|(id, e)| (e.local_path.as_str(), (*id, e))).collect();

    // Files in fs_base but not on disk → Deleted
    for (id, entry) in fs_base {
        if !local_tree.contains_key(&entry.local_path) {
            changes.push(LocalChange::Deleted { id: *id });
        }
    }

    // Files on disk
    for (path, local_info) in local_tree {
        match base_by_path.get(path.as_str()) {
            None => {
                changes.push(LocalChange::NewFile {
                    path: path.clone(),
                    is_dir: local_info.is_dir,
                });
            }
            Some((id, base_entry)) => {
                if !local_info.is_dir && local_info.content_hash != base_entry.content_hash {
                    changes.push(LocalChange::Modified { path: path.clone(), id: *id });
                }
            }
        }
    }

    changes.sort_by(|a, b| change_sort_key(a).cmp(&change_sort_key(b)));
    changes
}

fn change_sort_key(change: &LocalChange) -> (u8, isize) {
    match change {
        LocalChange::NewFile { path, is_dir: true } => (0, path_depth(path) as isize),
        LocalChange::NewFile { path, is_dir: false } => (1, path_depth(path) as isize),
        LocalChange::Modified { path, .. } => (1, path_depth(path) as isize),
        LocalChange::Deleted { .. } => (2, 0),
    }
}

fn path_depth(path: &str) -> usize {
    path.chars().filter(|c| *c == '/').count()
}

// --- Step 3: apply local changes to lb-rs ---

async fn apply_local_to_lb(
    lb: &Lb,
    root_path: &str,
    changes: &[LocalChange],
    local_dir: &Path,
) -> Result<(), SyncDirError> {
    for change in changes {
        match change {
            LocalChange::NewFile { path, is_dir: true } => {
                let full = format!("{root_path}/{path}/");
                match lb.create_at_path(&full).await {
                    Ok(_) => tracing::info!("creating remote dir: {path}"),
                    Err(e)
                        if matches!(
                            &e.kind,
                            LbErrKind::Validation(ValidationFailure::PathConflict(_))
                        ) =>
                    {
                        tracing::debug!("remote dir already exists: {path}");
                    }
                    Err(e) => return Err(SyncDirError::Lb(e.kind)),
                }
            }

            LocalChange::NewFile { path, is_dir: false } => {
                let content = fs::read(local_dir.join(path))?;
                let full = format!("{root_path}/{path}");
                tracing::info!("pushing new file: {path}");
                match lb.create_at_path(&full).await {
                    Ok(file) => {
                        lb.write_document(file.id, &content)
                            .await
                            .map_err(|e| SyncDirError::Lb(e.kind))?;
                    }
                    Err(e)
                        if matches!(
                            &e.kind,
                            LbErrKind::Validation(ValidationFailure::PathConflict(_))
                        ) =>
                    {
                        tracing::debug!("remote file already exists, updating: {path}");
                        let existing = lb
                            .get_by_path(&full)
                            .await
                            .map_err(|e| SyncDirError::Lb(e.kind))?;
                        lb.write_document(existing.id, &content)
                            .await
                            .map_err(|e| SyncDirError::Lb(e.kind))?;
                    }
                    Err(e) => return Err(SyncDirError::Lb(e.kind)),
                }
            }

            LocalChange::Modified { path, id } => {
                tracing::info!("updating remote: {path}");
                let content = fs::read(local_dir.join(path))?;
                lb.write_document(*id, &content)
                    .await
                    .map_err(|e| SyncDirError::Lb(e.kind))?;
            }

            LocalChange::Deleted { id } => {
                tracing::info!("deleting remote: {id}");
                lb.delete(id).await.map_err(|e| SyncDirError::Lb(e.kind))?;
            }
        }
    }

    Ok(())
}

// --- Step 5: materialize lb-rs state to disk ---

async fn materialize_to_disk(
    lb: &Lb,
    local_dir: &Path,
    ignore: &IgnoreRules,
    remote_tree: &[(String, File)],
    local_tree: &HashMap<String, LocalFileInfo>,
    fs_base: &HashMap<Uuid, FsBaseEntry>,
) -> Result<(), SyncDirError> {
    let remote_by_path: HashMap<&str, &File> =
        remote_tree.iter().map(|(path, file)| (path.as_str(), file)).collect();

    let base_by_id: HashMap<Uuid, &FsBaseEntry> = fs_base.iter().map(|(id, e)| (*id, e)).collect();

    for (rel_path, file) in remote_tree {
        let full_path = local_dir.join(rel_path);
        if ignore.is_ignored(&full_path, file.is_folder()) {
            continue;
        }

        if file.is_folder() {
            if !full_path.exists() {
                tracing::info!("creating local dir: {rel_path}");
                fs::create_dir_all(&full_path)?;
            }
            continue;
        }

        // Optimization: skip if last_modified matches fs_base (unchanged since last agreement)
        if let Some(base) = base_by_id.get(&file.id) {
            if file.last_modified == base.lb_last_modified {
                if let Some(local) = local_tree.get(rel_path.as_str()) {
                    if local.content_hash == base.content_hash {
                        continue;
                    }
                }
            }
        }

        let content = lb
            .read_document(file.id, false)
            .await
            .map_err(|e| SyncDirError::Lb(e.kind))?;
        let remote_hash = hash_bytes(&content);

        if let Some(local_info) = local_tree.get(rel_path.as_str()) {
            if remote_hash == local_info.content_hash {
                continue;
            }

            // Write-race check: did the disk change since fs_base?
            if let Some(base) = base_by_id.get(&file.id) {
                if local_info.content_hash != base.content_hash {
                    tracing::info!(
                        "write-race conflict: {rel_path} — saving local as sidecar",
                    );
                    let local_content = fs::read(local_dir.join(rel_path))?;
                    write_conflict_sidecar(local_dir, rel_path, &local_content)?;
                }
            }

            tracing::info!("updating local: {rel_path}");
            write_local_file(local_dir, rel_path, &content)?;
        } else {
            tracing::info!("pulling: {rel_path}");
            write_local_file(local_dir, rel_path, &content)?;
        }
    }

    // Delete local files not in remote tree (only if previously tracked)
    for (path, _) in local_tree {
        if remote_by_path.contains_key(path.as_str()) {
            continue;
        }
        let was_tracked = fs_base.values().any(|e| e.local_path == *path);
        if was_tracked {
            tracing::info!("deleting local (remotely deleted): {path}");
            delete_local(local_dir, path)?;
        }
    }

    Ok(())
}

// --- Helpers ---

/// Get the remote file tree under the sync root, using lb-rs path APIs.
/// Returns `(relative_path, File)` pairs with paths relative to the sync root.
async fn get_remote_tree(lb: &Lb, root_path: &str) -> Result<Vec<(String, File)>, SyncDirError> {
    let prefix = format!("{root_path}/");

    // 1. Get all paths, build id→relative_path map for our subtree
    let all_paths = lb
        .list_paths_with_ids(None)
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?;
    let mut path_by_id: HashMap<Uuid, String> = HashMap::new();
    for (id, path) in all_paths {
        if let Some(rel) = path.strip_prefix(&prefix) {
            let rel = rel.trim_end_matches('/');
            if !rel.is_empty() {
                path_by_id.insert(id, rel.to_string());
            }
        }
    }

    // 2. Get all descendant Files in one call
    let root_id = lb
        .get_by_path(root_path)
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?
        .id;
    let descendants = lb
        .get_and_get_children_recursively(&root_id)
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?;

    // 3. Join: pair each descendant with its relative path
    let mut result = Vec::new();
    for file in descendants {
        if let Some(rel_path) = path_by_id.remove(&file.id) {
            result.push((rel_path, file));
        }
    }
    Ok(result)
}

fn build_new_fs_base(
    local_dir: &Path,
    ignore: &IgnoreRules,
    remote_tree: &[(String, File)],
) -> Result<Vec<(Uuid, FsBaseEntry)>, SyncDirError> {
    let mut entries = Vec::new();

    for (rel_path, file) in remote_tree {
        if file.is_folder() {
            continue;
        }

        let local_path = local_dir.join(rel_path);
        if ignore.is_ignored(&local_path, false) {
            continue;
        }

        let content_hash = if local_path.exists() {
            hash_file(&local_path).unwrap_or([0; 32])
        } else {
            [0; 32]
        };

        entries.push((
            file.id,
            FsBaseEntry {
                local_path: rel_path.clone(),
                content_hash,
                lb_last_modified: file.last_modified,
            },
        ));
    }

    Ok(entries)
}

async fn resolve_or_create_lb_folder(
    lb: &Lb,
    folder_path: &str,
) -> Result<String, SyncDirError> {
    let file = match lb.get_by_path(folder_path).await {
        Ok(file) => file,
        Err(_) => {
            tracing::info!("creating lockbook folder: {folder_path}");
            lb.create_at_path(&format!("{folder_path}/"))
                .await
                .map_err(|e| SyncDirError::Lb(e.kind))?
        }
    };
    let path = lb
        .get_path_by_id(file.id)
        .await
        .map_err(|e| SyncDirError::Lb(e.kind))?;
    Ok(path.trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_entry(path: &str, hash: [u8; 32], modified: u64) -> FsBaseEntry {
        FsBaseEntry { local_path: path.to_string(), content_hash: hash, lb_last_modified: modified }
    }

    fn local_file(path: &str, hash: [u8; 32]) -> (String, LocalFileInfo) {
        (
            path.to_string(),
            LocalFileInfo { content_hash: hash, is_dir: false },
        )
    }

    fn local_dir_entry(path: &str) -> (String, LocalFileInfo) {
        (
            path.to_string(),
            LocalFileInfo { content_hash: [0; 32], is_dir: true },
        )
    }

    #[test]
    fn no_changes_when_in_sync() {
        let id = Uuid::new_v4();
        let hash = [1u8; 32];
        let fs_base: HashMap<_, _> = [(id, base_entry("a.txt", hash, 100))].into();
        let local_tree: HashMap<_, _> = [local_file("a.txt", hash)].into();

        let changes = detect_local_changes(&local_tree, &fs_base);
        assert!(changes.is_empty());
    }

    #[test]
    fn detects_local_modification() {
        let id = Uuid::new_v4();
        let old_hash = [1u8; 32];
        let new_hash = [2u8; 32];
        let fs_base: HashMap<_, _> = [(id, base_entry("a.txt", old_hash, 100))].into();
        let local_tree: HashMap<_, _> = [local_file("a.txt", new_hash)].into();

        let changes = detect_local_changes(&local_tree, &fs_base);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], LocalChange::Modified { id: mid, .. } if *mid == id));
    }

    #[test]
    fn detects_new_local_file() {
        let fs_base: HashMap<Uuid, FsBaseEntry> = HashMap::new();
        let local_tree: HashMap<_, _> = [local_file("new.txt", [3u8; 32])].into();

        let changes = detect_local_changes(&local_tree, &fs_base);
        assert_eq!(changes.len(), 1);
        assert!(
            matches!(&changes[0], LocalChange::NewFile { path, is_dir: false } if path == "new.txt")
        );
    }

    #[test]
    fn detects_local_deletion() {
        let id = Uuid::new_v4();
        let hash = [1u8; 32];
        let fs_base: HashMap<_, _> = [(id, base_entry("a.txt", hash, 100))].into();
        let local_tree: HashMap<String, LocalFileInfo> = HashMap::new();

        let changes = detect_local_changes(&local_tree, &fs_base);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], LocalChange::Deleted { id: did } if *did == id));
    }

    #[test]
    fn dirs_created_before_files() {
        let fs_base: HashMap<Uuid, FsBaseEntry> = HashMap::new();
        let local_tree: HashMap<_, _> =
            [local_file("src/main.rs", [1u8; 32]), local_dir_entry("src")].into();

        let changes = detect_local_changes(&local_tree, &fs_base);
        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], LocalChange::NewFile { is_dir: true, .. }));
        assert!(matches!(&changes[1], LocalChange::NewFile { is_dir: false, .. }));
    }
}
