use super::hash::hash_file;
use super::ignore::IgnoreRules;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalFileInfo {
    pub relative_path: String,
    pub content_hash: [u8; 32],
    pub is_dir: bool,
}

/// Scan the local directory tree, returning a map of relative_path -> LocalFileInfo.
/// Follows symlinks. Detects symlink cycles via visited (dev, ino) pairs.
pub fn scan_local_tree(
    root: &Path,
    ignore: &IgnoreRules,
) -> io::Result<HashMap<String, LocalFileInfo>> {
    let mut result = HashMap::new();
    let mut visited_dirs = HashSet::new();
    scan_recursive(root, root, ignore, &mut result, &mut visited_dirs)?;
    Ok(result)
}

fn scan_recursive(
    root: &Path,
    current: &Path,
    ignore: &IgnoreRules,
    result: &mut HashMap<String, LocalFileInfo>,
    visited_dirs: &mut HashSet<(u64, u64)>,
) -> io::Result<()> {
    // Cycle detection using device + inode
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::metadata(current)?;
        if meta.is_dir() {
            let key = (meta.dev(), meta.ino());
            if !visited_dirs.insert(key) {
                tracing::warn!("symlink cycle detected at {}, skipping", current.display());
                return Ok(());
            }
        }
    }

    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("cannot read directory {}: {e}", current.display());
            return Ok(());
        }
    };

    for entry in entries {
        let entry = entry?;
        // Use metadata() which follows symlinks, not symlink_metadata()
        let meta = match fs::metadata(entry.path()) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("cannot stat {}: {e}", entry.path().display());
                continue;
            }
        };

        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();

        if ignore.is_ignored(&entry.path(), meta.is_dir()) {
            continue;
        }

        if meta.is_dir() {
            result.insert(
                rel.clone(),
                LocalFileInfo { relative_path: rel, content_hash: [0; 32], is_dir: true },
            );
            scan_recursive(root, &entry.path(), ignore, result, visited_dirs)?;
        } else if meta.is_file() {
            let content_hash = hash_file(&entry.path()).unwrap_or([0; 32]);
            result.insert(
                rel.clone(),
                LocalFileInfo { relative_path: rel, content_hash, is_dir: false },
            );
        }
    }

    Ok(())
}

/// Write file content to a local path, creating parent directories as needed.
/// Uses atomic write (temp file + rename) to avoid partial writes.
pub fn write_local_file(root: &Path, relative_path: &str, content: &[u8]) -> io::Result<()> {
    let target = root.join(relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let dir = target.parent().unwrap_or(root);
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    fs::write(tmp.path(), content)?;
    tmp.persist(&target).map_err(|e| e.error)?;
    Ok(())
}

/// Create a conflict sidecar: `<stem>.conflict-<timestamp>.<ext>`
pub fn write_conflict_sidecar(
    root: &Path,
    relative_path: &str,
    content: &[u8],
) -> io::Result<PathBuf> {
    let original = Path::new(relative_path);
    let stem = original
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let ext = original.extension().map(|e| e.to_string_lossy().to_string());
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S");

    let sidecar_name = match ext {
        Some(e) => format!("{stem}.conflict-{timestamp}.{e}"),
        None => format!("{stem}.conflict-{timestamp}"),
    };

    let sidecar_rel = match original.parent() {
        Some(p) if p != Path::new("") => {
            p.join(&sidecar_name).to_string_lossy().to_string()
        }
        _ => sidecar_name,
    };

    let sidecar_path = root.join(&sidecar_rel);
    if let Some(parent) = sidecar_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&sidecar_path, content)?;
    Ok(sidecar_path)
}

/// Delete a local file or directory.
pub fn delete_local(root: &Path, relative_path: &str) -> io::Result<()> {
    let target = root.join(relative_path);
    if target.is_dir() {
        fs::remove_dir_all(&target)?;
    } else if target.exists() {
        fs::remove_file(&target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_basic_tree() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("sub/b.txt"), "world").unwrap();

        let ignore = IgnoreRules::load(dir.path());
        let tree = scan_local_tree(dir.path(), &ignore).unwrap();

        assert!(tree.contains_key("a.txt"));
        assert!(tree.contains_key("sub/b.txt"));
        assert!(tree.contains_key("sub"));
        assert!(tree.get("sub").unwrap().is_dir);
        assert!(!tree.get("a.txt").unwrap().is_dir);
    }

    #[test]
    fn scan_respects_ignore() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/foo.js"), "x").unwrap();
        fs::write(dir.path().join("keep.txt"), "y").unwrap();

        let ignore = IgnoreRules::load(dir.path());
        let tree = scan_local_tree(dir.path(), &ignore).unwrap();

        assert!(!tree.contains_key("node_modules"));
        assert!(!tree.contains_key("node_modules/foo.js"));
        assert!(tree.contains_key("keep.txt"));
    }

    #[test]
    fn conflict_sidecar_naming() {
        let dir = tempfile::tempdir().unwrap();
        let path =
            write_conflict_sidecar(dir.path(), "src/main.rs", b"conflict content").unwrap();

        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("main.conflict-"));
        assert!(name.ends_with(".rs"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "conflict content");
    }

    #[test]
    fn atomic_write_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        write_local_file(dir.path(), "deep/nested/file.txt", b"content").unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("deep/nested/file.txt")).unwrap(),
            "content"
        );

        delete_local(dir.path(), "deep/nested/file.txt").unwrap();
        assert!(!dir.path().join("deep/nested/file.txt").exists());
    }
}
