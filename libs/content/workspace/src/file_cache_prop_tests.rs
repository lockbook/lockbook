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
//! - **Wikilinks can't find their target** because an omitted extension isn't
//!   matched (`note` should find `note.md`).
//! - **Wikilinks silently resolve an ambiguous stem** (`note` with both
//!   `note.md` and `note.svg` present) instead of returning None.
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

use crate::file_cache::{FileCache, FilesExt, ResolvedLink, relative_path, strip_ext};
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

/// Document extensions drawn from for generated documents. The empty string
/// models extension-less files; the mix of extensions produces sibling stem
/// collisions (e.g. `a.md` + `a.svg`) that exercise wikilink ambiguity.
const DOC_EXTS: [&str; 4] = [".md", ".txt", ".svg", ""];

/// Picks a name and type: 50/50 folder/document, name drawn from `POOL`.
/// Documents get an extension drawn from `DOC_EXTS` (possibly none).
fn pick_file(src: &mut ByteSource) -> (String, FileType) {
    let is_folder = src.bias(&[1, 1]) == 1;
    let c = POOL[src.draw(POOL.len())];
    if is_folder {
        (c.to_string(), FileType::Folder)
    } else {
        (format!("{c}{}", DOC_EXTS[src.draw(DOC_EXTS.len())]), FileType::Document)
    }
}

/// Fills descendants under `root_id` (already in `out`), skipping any sibling
/// name collision since files can't have path conflicts. Sibling documents that
/// share a stem but differ by extension are allowed — they make bare titles
/// ambiguous, which the wikilink suite checks.
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
        files: files.into_iter().map(|f| (f.id, f)).collect(),
        shared: shared.into_iter().map(|f| (f.id, f)).collect(),
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

/// Wikilinks resolve within the same tree by case-insensitive title match.
/// Extensions are optional in the link (`note` matches `note.md`) but never
/// stripped from the file, so an exact full name wins and stem collisions are
/// ambiguous. Paths disambiguate via the folder relative to `from_id`.
fn wikilink_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let cache = cache(&mut src);
    let folders: Vec<&File> = cache.iter_files().filter(|f| f.is_folder()).collect();
    let from_id = folders[src.draw(folders.len())].id;
    let from_path = cache.path(from_id);
    let docs: Vec<&File> = cache.iter_files().filter(|f| f.is_document()).collect();

    // Count of same-tree-as-`from_id` docs satisfying a name predicate.
    let tree_count = |pred: &dyn Fn(&str) -> bool| {
        docs.iter()
            .filter(|d| cache.same_tree(from_id, d.id))
            .filter(|d| pred(&d.name))
            .count()
    };

    for f in &docs {
        let full = f.name.as_str();
        let stem = strip_ext(full);
        let same = cache.same_tree(from_id, f.id);

        // A: a doc's full name resolves to itself from its own parent. (Names
        // are unique within a folder, so the exact match is unambiguous.) Skip
        // share-root docs whose parent isn't a visible folder.
        if cache.get_by_id(f.parent).is_some()
            && !matches!(cache.resolve_wikilink(full, f.parent), Some(id) if id == f.id)
        {
            return Err("A: full-name own-parent self-resolve");
        }

        // B: the full relative path resolves to the doc (same tree).
        if same {
            let rel = relative_path(&from_path, &cache.path(f.id));
            if !matches!(cache.resolve_wikilink(&rel, from_id), Some(id) if id == f.id) {
                return Err("B: full relative-path round-trip");
            }
        }

        // C: a path with the extension dropped resolves when the stem is unique
        // within the doc's folder.
        if same {
            let folder_stem_unique = cache
                .children(f.parent)
                .into_iter()
                .filter(|d| d.is_document())
                .filter(|d| strip_ext(&d.name).eq_ignore_ascii_case(stem))
                .count()
                == 1;
            let rel = relative_path(&from_path, &cache.path(f.id));
            if folder_stem_unique && rel.contains('/') {
                let (dir, _) = rel.rsplit_once('/').unwrap();
                let rel_stem = format!("{dir}/{stem}");
                if !matches!(cache.resolve_wikilink(&rel_stem, from_id), Some(id) if id == f.id) {
                    return Err("C: stem relative-path round-trip");
                }
            }
        }

        // D: a globally-unique bare stem resolves to its doc from anywhere.
        if same
            && tree_count(&|n| strip_ext(n).eq_ignore_ascii_case(stem)) == 1
            && !matches!(cache.resolve_wikilink(stem, from_id), Some(id) if id == f.id)
        {
            return Err("D: unique-stem bare resolve");
        }

        // E: a globally-unique bare full name resolves to its doc from anywhere.
        if same
            && tree_count(&|n| n.eq_ignore_ascii_case(full)) == 1
            && !matches!(cache.resolve_wikilink(full, from_id), Some(id) if id == f.id)
        {
            return Err("E: unique-name bare resolve");
        }
    }

    // F: soundness — any resolution returns a same-tree doc whose name matches
    // the title (exactly or stem). Probe every doc's stem and full name.
    for title in docs
        .iter()
        .flat_map(|f| [strip_ext(&f.name).to_string(), f.name.clone()])
    {
        if let Some(id) = cache.resolve_wikilink(&title, from_id) {
            let Some(r) = cache.get_by_id(id) else {
                return Err("F: resolved id not in cache");
            };
            if !(r.name.eq_ignore_ascii_case(&title)
                || strip_ext(&r.name).eq_ignore_ascii_case(&title))
            {
                return Err("F: resolved file doesn't match title");
            }
            if !cache.same_tree(from_id, id) {
                return Err("F: resolved wikilink is cross-tree");
            }
        }
    }

    // G: ambiguity — a stem shared by 2+ docs directly in a folder, with no doc
    // named exactly that stem in the same tree, must not resolve from that
    // folder (both are equally near; the link needs an extension).
    for folder in cache.iter_files().filter(|f| f.is_folder()) {
        let child_docs: Vec<&File> = cache
            .children(folder.id)
            .into_iter()
            .filter(|d| d.is_document())
            .collect();
        for d in &child_docs {
            let stem = strip_ext(&d.name);
            let shared = child_docs
                .iter()
                .filter(|x| strip_ext(&x.name).eq_ignore_ascii_case(stem))
                .count()
                >= 2;
            let exact_exists = docs
                .iter()
                .filter(|x| cache.same_tree(folder.id, x.id))
                .any(|x| x.name.eq_ignore_ascii_case(stem));
            if shared && !exact_exists && cache.resolve_wikilink(stem, folder.id).is_some() {
                return Err("G: ambiguous stem must not resolve");
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
