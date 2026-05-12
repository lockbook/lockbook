//! Property tests for `FilesExt::resolve_link` and `resolve_wikilink`.
//!
//! # Link resolution policy
//!
//! **Own tree** (user's own root and any accepted shares):
//! - Absolute path links valid (no path points outside own tree).
//! - Relative path links valid (no path points outside own tree).
//! - UUID links valid regardless destination.
//! - Wikilinks resolve within tree only.
//!
//! **Pending share trees**:
//! - Absolute path invalid (pending shares don't have absolute paths).
//! - Relative path links resolve within tree only.
//! - UUID links valid regardless destination (LinkState::Warning if cross-tree)
//! - Wikilinks resolve within tree only.
//!
//! # Test structure
//!
//! Each seed produces a `FileCache` with one own tree and 0–3 pending shares.
//! Names are drawn from a shared pool, so cross-tree collisions are common.
//! Nested shared folders are out of scope — see
//! <https://github.com/lockbook/lockbook/issues/4496>. On failure the buffer
//! is delta-debugged and the shrunken case is printed.
//!
//! # Breakages this suite detects
//!
//! Confirmed via fault injection:
//!
//! - **Tree isolation is violated** — absolute paths resolving across trees,
//!   or wikilinks matching documents in a different tree.
//! - **`path()` produces wrong strings** — components in reverse order, or
//!   own-tree paths missing their leading `/`.
//! - **Percent-encoded absolute paths fail to resolve.**
//! - **Wikilinks can't find their target** because `.md` isn't trimmed from
//!   filenames before comparing to the title.
//! - **Folder paths resolve as documents** (should return None).
//! - **Wikilink ties are resolved toward the farthest match** instead of the
//!   nearest.
//! - **Excessive `..` in a relative path saturates silently** at the tree
//!   root instead of returning None.

use std::collections::HashMap;

use lb_rs::Uuid;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use rand::{Rng, SeedableRng, rngs::StdRng};
use urlencoding::encode as percent_encode;

use crate::file_cache::{FileCache, FilesExt, ResolvedLink, relative_path};
use crate::test_utils::byte_source::ByteSource;
use crate::test_utils::shrink::shrink;

/// Builds a `File` with zero-valued defaults for fields the tests don't use.
fn file(id: Uuid, parent: Uuid, name: &str, file_type: FileType) -> File {
    File {
        id,
        parent,
        name: name.into(),
        file_type,
        last_modified: 0,
        last_modified_by: String::new(),
        owner: String::new(),
        shares: vec![],
        size_bytes: 0,
    }
}

const POOL: [&str; 5] = ["a", "b", "c", "d", "a b"];

/// Weights for picking how many files to add in a subtree (max iterations =
/// length - 1). Bounds tree depth, since each iteration can add at most one
/// level when every file is a folder in a straight chain.
const SUBTREE_SIZE_BIAS: &[u32] = &[2, 3, 4, 4, 3, 2, 2, 1];

/// Upper bound on own-tree depth: derived from `SUBTREE_SIZE_BIAS` on the
/// assumption that every added file is a folder extending the deepest chain.
const MAX_TREE_DEPTH: usize = SUBTREE_SIZE_BIAS.len() - 1;

/// Picks a name and type: 50/50 folder/document, name drawn from `POOL`.
/// Documents get `.md` appended.
fn pick_file(src: &mut ByteSource) -> (String, FileType) {
    let is_folder = src.bias(&[1, 1]) == 1;
    let c = POOL[src.draw(POOL.len())];
    if is_folder {
        (c.to_string(), FileType::Folder)
    } else {
        (format!("{c}.md"), FileType::Document)
    }
}

/// Fills descendants under `root_id` (already in `out`), skipping any sibling
/// name collision since files can't have path conflicts.
fn fill_subtree(out: &mut Vec<File>, src: &mut ByteSource, root_id: Uuid) {
    let mut folders = vec![root_id];
    for _ in 0..src.bias(SUBTREE_SIZE_BIAS) {
        let parent = folders[src.draw(folders.len())];
        let (name, file_type) = pick_file(src);
        if out.iter().any(|f| f.parent == parent && f.name == name) {
            continue;
        }
        let id = Uuid::new_v4();
        if matches!(file_type, FileType::Folder) {
            folders.push(id);
        }
        out.push(file(id, parent, &name, file_type));
    }
}

/// Generates a `FileCache` from `src`: one own tree plus 0–3 disjoint pending
/// shares. Every file's name is drawn from the same pool.
fn cache(src: &mut ByteSource) -> FileCache {
    // Own tree: a self-parenting root and its descendants.
    let own_root_id = Uuid::new_v4();
    let own_root = file(own_root_id, own_root_id, "root", FileType::Folder);
    let mut files = vec![own_root.clone()];
    fill_subtree(&mut files, src, own_root_id);

    // 0–3 pending shares. Each share root's parent is a fresh UUID that doesn't
    // appear anywhere else — modeling the owner's file we don't have access to.
    // The share root can be a folder (with filled-in descendants) or a single
    // document. Its name comes from the same pool as everything else, so
    // cross-tree name collisions are common.
    let mut shared = vec![];
    for _ in 0..src.bias(&[6, 3, 2, 1]) {
        let absent_parent = Uuid::new_v4();
        let share_root_id = Uuid::new_v4();
        let (name, file_type) = pick_file(src);
        let is_folder = matches!(file_type, FileType::Folder);
        shared.push(file(share_root_id, absent_parent, &name, file_type));
        if is_folder {
            fill_subtree(&mut shared, src, share_root_id);
        }
    }

    FileCache {
        root: own_root,
        files,
        shared,
        suggested: vec![],
        size_bytes_recursive: HashMap::new(),
        last_modified_recursive: HashMap::new(),
        last_modified_by_recursive: HashMap::new(),
        last_modified: 0,
        shared_roots: vec![],
    }
}

/// Round-trips absolute / relative / percent-encoded paths for own-tree docs,
/// and checks that folder paths don't resolve and excess `..` doesn't escape.
fn link_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let cache = cache(&mut src);
    let own_root = cache.root().id;
    let folders: Vec<&File> = cache.iter_files().filter(|f| f.is_folder()).collect();
    let from_id = folders[src.draw(folders.len())].id;
    let from_path = cache.path(from_id);
    let from_is_own_tree = cache.tree_root(from_id) == own_root;
    for f in cache.iter_files().filter(|f| f.is_document()) {
        let f_is_own_tree = cache.tree_root(f.id) == own_root;
        let same_tree = cache.same_tree(from_id, f.id);
        let abs = cache.path(f.id);
        // A: own-tree doc's absolute path resolves to itself from any own-tree folder
        if f_is_own_tree
            && from_is_own_tree
            && !matches!(cache.resolve_link(&abs, from_id), Some(ResolvedLink::File(id)) if id == f.id)
        {
            return Err("absolute round-trip");
        }
        // B: doc's path relative to `from_id` resolves to itself (same tree only)
        if same_tree {
            let rel = relative_path(&from_path, &abs);
            if !matches!(cache.resolve_link(&rel, from_id), Some(ResolvedLink::File(id)) if id == f.id)
            {
                return Err("relative round-trip");
            }
        }
        // C: percent-encoded absolute path resolves the same as the raw path
        if f_is_own_tree && from_is_own_tree {
            let encoded: String = abs
                .split('/')
                .map(|seg| percent_encode(seg).into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if !matches!(cache.resolve_link(&encoded, from_id), Some(ResolvedLink::File(id)) if id == f.id)
            {
                return Err("percent-encoded absolute round-trip");
            }
        }
    }
    // D: a path that points to a folder must not resolve (only documents do)
    if from_is_own_tree {
        for f in cache.iter_files().filter(|f| f.is_folder()) {
            if cache.tree_root(f.id) != own_root {
                continue;
            }
            let abs = cache.path(f.id);
            if cache.resolve_link(&abs, from_id).is_some() {
                return Err("absolute path to a folder must not resolve");
            }
        }
    }
    // E: excessive `..` in a relative path must not saturate at the tree root
    // and silently continue resolving — it must return None. Prefix an
    // existing own-tree doc's path with one more `..` than the tree's max
    // depth, guaranteeing the walk would escape. If `..` at root correctly
    // returns None the whole path fails; if it no-ops, the excess dots
    // saturate and the tail finds the doc from root.
    if from_is_own_tree {
        for target in cache
            .iter_files()
            .filter(|f| f.is_document())
            .filter(|f| cache.tree_root(f.id) == own_root)
        {
            let tail = cache.path(target.id);
            let tail = tail.trim_start_matches('/');
            let escape = format!("{}{tail}", "../".repeat(MAX_TREE_DEPTH + 1));
            if cache.resolve_link(&escape, from_id).is_some() {
                return Err("excessive `..` escaped the tree root");
            }
        }
    }
    Ok(())
}

/// Wikilinks resolve within the same tree by case-insensitive title match,
/// with disambiguation via a relative path.
fn wikilink_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let cache = cache(&mut src);
    let folders: Vec<&File> = cache.iter_files().filter(|f| f.is_folder()).collect();
    let from_id = folders[src.draw(folders.len())].id;
    let from_path = cache.path(from_id);
    for f in cache.iter_files().filter(|f| f.is_document()) {
        let title = f.name.trim_end_matches(".md");
        // A: from own parent (always same tree), doc resolves to itself.
        // Skip share-root documents whose parent isn't in the cache — "from own
        // parent" isn't meaningful when the parent isn't a folder we can see.
        if cache.get_by_id(f.parent).is_some()
            && !matches!(cache.resolve_wikilink(title, f.parent), Some(id) if id == f.id)
        {
            return Err("A: own parent self-resolve");
        }
        // B: from any same-tree folder, resolves to a doc with matching title
        if cache.same_tree(from_id, f.id) {
            let Some(id) = cache.resolve_wikilink(title, from_id) else {
                return Err("B: lookup returned None");
            };
            let Some(r) = cache.get_by_id(id) else {
                return Err("B: resolved id not in cache");
            };
            if !r.name.trim_end_matches(".md").eq_ignore_ascii_case(title) {
                return Err("B: resolved title mismatch");
            }
            // C: resolved id is always same-tree as from_id
            if !cache.same_tree(from_id, id) {
                return Err("C: resolved wikilink is cross-tree");
            }
        }
        // D: disambiguation path (relative path containing /) round-trips same-tree
        if cache.same_tree(from_id, f.id) {
            let abs = cache.path(f.id);
            let rel = relative_path(&from_path, &abs);
            if rel.contains('/') {
                let disambiguation = rel.trim_end_matches(".md");
                if !matches!(cache.resolve_wikilink(disambiguation, from_id), Some(id) if id == f.id)
                {
                    return Err("D: disambiguation round-trip");
                }
            }
        }
    }
    Ok(())
}

/// UUID links always resolve to a `File`; path-based links never cross tree
/// boundaries in either direction.
fn cross_tree_policy_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let cache = cache(&mut src);
    let own_root = cache.root().id;
    let folders: Vec<&File> = cache.iter_files().filter(|f| f.is_folder()).collect();
    let from_id = folders[src.draw(folders.len())].id;
    let from_is_own = cache.tree_root(from_id) == own_root;

    for f in cache.iter_files().filter(|f| f.is_document()) {
        let f_is_own = cache.tree_root(f.id) == own_root;
        let same = cache.same_tree(from_id, f.id);

        // UUID links always resolve to File regardless of tree boundary
        let uuid_url = format!("lb://{}", f.id);
        if !matches!(cache.resolve_link(&uuid_url, from_id), Some(ResolvedLink::File(id)) if id == f.id)
        {
            return Err("uuid: must always resolve to File");
        }

        // Absolute path from share-tree folder must not resolve (even to own-tree docs)
        if !from_is_own && f_is_own {
            let abs = cache.path(f.id); // starts with /
            if cache.resolve_link(&abs, from_id).is_some() {
                return Err("abs path from share-tree must not resolve");
            }
        }

        // Relative path resolves iff source and target are in the same tree.
        // (The "both own-tree but different" case can't happen: own tree is one tree.)
        if same {
            let from_path = cache.path(from_id);
            let abs = cache.path(f.id);
            let rel = relative_path(&from_path, &abs);
            if !matches!(cache.resolve_link(&rel, from_id), Some(ResolvedLink::File(id)) if id == f.id)
            {
                return Err("rel path: same-tree must resolve");
            }
        } else {
            // relative path computed toward a cross-tree file must not reach that file
            let from_path = cache.path(from_id);
            let abs = cache.path(f.id);
            let rel = relative_path(&from_path, &abs);
            if matches!(cache.resolve_link(&rel, from_id), Some(ResolvedLink::File(id)) if id == f.id)
            {
                return Err("rel path: must not resolve to a cross-tree file");
            }
        }
    }
    Ok(())
}

/// Runs `check` across 2048 seeded buffers. On failure, delta-debugs the
/// input and panics with the shrunken buffer and its reconstructed cache.
fn run(check: fn(&[u8]) -> Result<(), &'static str>) {
    for seed in 0..2048u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut buf = vec![0u8; 128];
        rng.fill(&mut buf[..]);
        if let Err(reason) = check(&buf) {
            let shrunk = shrink(buf, |b| check(b).is_err());
            let mut src = ByteSource::new(&shrunk);
            let cache = cache(&mut src);
            panic!(
                "seed {seed} {reason}\nshrunk ({} bytes): {shrunk:?}\nfiles:\n{:#?}\nshared:\n{:#?}",
                shrunk.len(),
                cache.files,
                cache.shared,
            );
        }
    }
}

#[test]
fn resolve_link_round_trip() {
    run(link_check);
}

#[test]
fn resolve_wikilink_round_trip() {
    run(wikilink_check);
}

#[test]
fn cross_tree_link_policy() {
    run(cross_tree_policy_check);
}
