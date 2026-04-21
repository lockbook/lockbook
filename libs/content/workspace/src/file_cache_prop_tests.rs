//! Property tests for `FilesExt::resolve_link` and `resolve_wikilink`.
//!
//! Both tests round-trip every reachable document through the resolver.
//! Names are drawn from a small pool (`a`/`b`/`c`/`d`) so cross-parent
//! conflicts are common; within-parent collisions are skipped to keep
//! sibling names distinct. On failure, the buffer is delta-debugged and
//! the shrunken case is printed alongside the resulting tree.

use lb_rs::Uuid;
use lb_rs::model::file::File;
use lb_rs::model::file_metadata::FileType;
use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::file_cache::{FilesExt, ResolvedLink, relative_path};
use crate::test_utils::byte_source::ByteSource;
use crate::test_utils::shrink::shrink;

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

fn fill_subtree(files: &mut Vec<File>, src: &mut ByteSource, root_id: Uuid) {
    let mut folders = vec![root_id];
    let pool = ['a', 'b', 'c', 'd'];
    for _ in 0..src.bias(&[2, 3, 4, 4, 3, 2, 2, 1]) {
        let parent = folders[src.draw(folders.len())];
        let is_folder = src.bias(&[3, 1]) == 1;
        let c = pool[src.draw(pool.len())];
        let (name, file_type) = if is_folder {
            (c.to_string(), FileType::Folder)
        } else {
            (format!("{c}.md"), FileType::Document)
        };
        if files.iter().any(|f| f.parent == parent && f.name == name) {
            continue;
        }
        let id = Uuid::new_v4();
        if is_folder {
            folders.push(id);
        }
        files.push(file(id, parent, &name, file_type));
    }
}

fn tree(src: &mut ByteSource) -> Vec<File> {
    let mut files = vec![];
    let own_root = Uuid::new_v4();
    files.push(file(own_root, own_root, "root", FileType::Folder));
    fill_subtree(&mut files, src, own_root);

    // share roots: parent UUID is phantom (not in cache).
    for i in 0..src.bias(&[6, 3, 2, 1]) {
        let phantom_parent = Uuid::new_v4();
        let share_root = Uuid::new_v4();
        files.push(file(share_root, phantom_parent, &format!("s{i}"), FileType::Folder));
        fill_subtree(&mut files, src, share_root);
    }
    files
}

fn link_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let tree = tree(&mut src);
    let folders: Vec<&File> = tree.iter().filter(|f| f.is_folder()).collect();
    let from_id = folders[src.draw(folders.len())].id;
    let from_path = tree.path(from_id);
    for f in tree.iter().filter(|f| f.is_document()) {
        let abs = tree.path(f.id);
        let rel = relative_path(&from_path, &abs);
        // A: doc's absolute path resolves to itself from any folder
        if !matches!(tree.resolve_link(&abs, from_id), Some(ResolvedLink::File(id)) if id == f.id) {
            return Err("absolute round-trip");
        }
        // B: doc's path relative to `from_id` resolves to itself
        if !matches!(tree.resolve_link(&rel, from_id), Some(ResolvedLink::File(id)) if id == f.id) {
            return Err("relative round-trip");
        }
    }
    Ok(())
}

fn wikilink_check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let tree = tree(&mut src);
    let folders: Vec<&File> = tree.iter().filter(|f| f.is_folder()).collect();
    let from_id = folders[src.draw(folders.len())].id;
    for f in tree.iter().filter(|f| f.is_document()) {
        let title = f.name.trim_end_matches(".md");
        // A: from own parent, doc resolves to itself (closest wins)
        if !matches!(tree.resolve_wikilink(title, f.parent), Some(id) if id == f.id) {
            return Err("A: own parent self-resolve");
        }
        // B: from any folder, resolves to some doc with matching title
        let Some(id) = tree.resolve_wikilink(title, from_id) else {
            return Err("B: lookup returned None");
        };
        let Some(r) = tree.get_by_id(id) else {
            return Err("B: resolved id not in cache");
        };
        if !r.name.trim_end_matches(".md").eq_ignore_ascii_case(title) {
            return Err("B: resolved title mismatch");
        }
    }
    Ok(())
}

fn report_and_panic(seed: u64, reason: &str, shrunk: Vec<u8>) -> ! {
    let mut src = ByteSource::new(&shrunk);
    let tree = tree(&mut src);
    panic!("seed {seed} {reason}\nshrunk ({} bytes): {shrunk:?}\ntree:\n{tree:#?}", shrunk.len(),);
}

fn run(check: fn(&[u8]) -> Result<(), &'static str>) {
    for seed in 0..2048u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut buf = vec![0u8; 128];
        rng.fill(&mut buf[..]);
        if let Err(reason) = check(&buf) {
            let shrunk = shrink(buf, |b| check(b).is_err());
            report_and_panic(seed, reason, shrunk);
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
