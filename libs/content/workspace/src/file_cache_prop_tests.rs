//! Property tests for `FilesExt::resolve_link` and `resolve_wikilink`.
//!
//! Each seed produces a world of three accounts whose trees share files with
//! one another, and each account's `FileCache` view of it: its own tree, plus
//! every subtree shared with it, either pending or placed somewhere in its
//! tree under a name of its choosing. Names are drawn from a shared pool, so
//! collisions across folders, shares, and accounts are common. On failure the
//! buffer is delta-debugged and the shrunken case is printed.
//!
//! # Invariants
//!
//! - **Same note, same answer** — a path or wiki destination resolves to the
//!   same file, or fails, in the view of every account that can read the note.
//! - **Scope** — path and wiki destinations resolve only inside the subtree of
//!   the note's innermost shared ancestor, which every view agrees on.
//! - **Round trip** — a relative or scope-anchored path to a document in the
//!   scope resolves to it; a path to a folder, or with excess `..`, does not.
//! - **The author's reach** — `lb://` resolves only to documents the note's
//!   owner can see, and always to ones the owner owns.
//! - **Written forms** — the destination or wiki title written for a document
//!   resolves back to it.
//! - **Wiki titles** — a resolved title names its document; a title unique in
//!   the scope resolves from anywhere in it; siblings sharing a stem are
//!   ambiguous without an extension.

use lb_rs::Uuid;
use lb_rs::model::file::{File, Share, ShareMode};
use lb_rs::model::file_metadata::FileType;
use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::file_cache::{FileCache, FilesExt, ResolvedLink, relative_path, strip_ext};
use crate::test_utils::byte_source::ByteSource;
use crate::test_utils::shrink::shrink;

const USERS: [&str; 3] = ["alice", "bob", "carol"];

const POOL: [&str; 5] = ["a", "b", "c", "d", "a b"];

/// Weights for picking how many files to add in a subtree (max iterations =
/// length - 1). Bounds tree depth, since each iteration can add at most one
/// level when every file is a folder in a straight chain.
const SUBTREE_SIZE_BIAS: &[u32] = &[2, 3, 4, 4, 3, 2, 2, 1];

/// Upper bound on a tree's depth, were every added file a folder extending
/// the deepest chain.
const MAX_TREE_DEPTH: usize = SUBTREE_SIZE_BIAS.len() - 1;

/// The empty string models extension-less files; the mix of extensions
/// produces sibling stem collisions (e.g. `a.md` + `a.svg`).
const DOC_EXTS: [&str; 4] = [".md", ".txt", ".svg", ""];

fn file(id: Uuid, parent: Uuid, name: &str, file_type: FileType, owner: &str) -> File {
    File {
        id,
        parent,
        name: name.into(),
        file_type,
        last_modified: 0,
        last_modified_by: String::new(),
        owner: owner.into(),
        shares: vec![],
        size_bytes: 0,
    }
}

/// Every account's files with their true parents.
struct World {
    files: Vec<File>,
}

impl World {
    fn get(&self, id: Uuid) -> &File {
        self.files.iter().find(|f| f.id == id).unwrap()
    }

    fn root(&self, user: &str) -> &File {
        let is_root = |f: &&File| f.is_root() && f.owner == user;
        self.files.iter().find(is_root).unwrap()
    }

    /// `id` and its ancestors, nearest first.
    fn lineage(&self, id: Uuid) -> Vec<&File> {
        let mut lineage = vec![self.get(id)];
        while !lineage.last().unwrap().is_root() {
            lineage.push(self.get(lineage.last().unwrap().parent));
        }
        lineage
    }

    fn shared_with(&self, user: &str, id: Uuid) -> bool {
        self.lineage(id)
            .iter()
            .any(|f| f.shares.iter().any(|s| s.shared_with == user))
    }

    fn visible(&self, user: &str, id: Uuid) -> bool {
        self.get(id).owner == user || self.shared_with(user, id)
    }
}

/// One tree per account, each file shared with zero, one, or both of the
/// other accounts.
fn world(src: &mut ByteSource) -> World {
    let mut files = vec![];
    for user in USERS {
        let root = Uuid::new_v4();
        files.push(file(root, root, user, FileType::Folder, user));
        let mut folders = vec![root];
        for _ in 0..src.bias(SUBTREE_SIZE_BIAS) {
            let parent = folders[src.draw(folders.len())];
            let name = POOL[src.draw(POOL.len())];
            let (name, file_type) = if src.bias(&[1, 1]) == 1 {
                (name.to_string(), FileType::Folder)
            } else {
                (format!("{name}{}", DOC_EXTS[src.draw(DOC_EXTS.len())]), FileType::Document)
            };
            if files.iter().any(|f| f.parent == parent && f.name == name) {
                continue;
            }
            let mut f = file(Uuid::new_v4(), parent, &name, file_type, user);
            if file_type == FileType::Folder {
                folders.push(f.id);
            }
            let others: Vec<&str> = USERS.into_iter().filter(|u| *u != user).collect();
            let recipients = match src.bias(&[6, 1, 1, 1]) {
                0 => &others[..0],
                1 => &others[..1],
                2 => &others[1..],
                _ => &others[..],
            };
            for with in recipients {
                f.shares.push(Share {
                    mode: ShareMode::Write,
                    shared_by: user.into(),
                    shared_with: with.to_string(),
                });
            }
            files.push(f);
        }
    }
    World { files }
}

/// `user`'s view: their tree plus each outermost subtree shared with them,
/// pending or placed at their root under a new name.
fn view(world: &World, src: &mut ByteSource, user: &str) -> FileCache {
    let root = world.root(user).clone();
    let (mut owned, mut pending) = (vec![], vec![]);
    let mut placed = vec![];
    for f in &world.files {
        if f.owner == user {
            owned.push(f.clone());
            continue;
        }
        let lineage = world.lineage(f.id);
        let Some(top) = lineage
            .iter()
            .rfind(|a| a.shares.iter().any(|s| s.shared_with == user))
        else {
            continue;
        };
        let mut f = f.clone();
        if f.id == top.id && src.draw(2) == 1 {
            placed.push(top.id);
            f.parent = root.id;
            f.name = format!("placed-{}", placed.len());
        }
        if placed.contains(&top.id) { owned.push(f) } else { pending.push(f) }
    }
    FileCache::from_owned_and_shared(root, owned, pending)
}

fn resolve(cache: &FileCache, url: &str, note: Uuid) -> Option<Uuid> {
    match cache.resolve_link(url, note)? {
        ResolvedLink::File(id) => Some(id),
        ResolvedLink::External(_) => None,
    }
}

fn check(buf: &[u8]) -> Result<(), &'static str> {
    let mut src = ByteSource::new(buf);
    let world = world(&mut src);
    let views: Vec<FileCache> = USERS.iter().map(|u| view(&world, &mut src, u)).collect();
    let docs: Vec<&File> = world.files.iter().filter(|f| f.is_document()).collect();

    for note in &docs {
        let author = USERS.iter().position(|u| *u == note.owner).unwrap();
        let base = &views[author];
        let readers: Vec<&FileCache> = USERS
            .iter()
            .zip(&views)
            .filter(|(u, _)| world.visible(u, note.id))
            .map(|(_, v)| v)
            .collect();

        let scope = base.scope_top(note.id);
        if readers.iter().any(|v| v.scope_top(note.id) != scope) {
            return Err("readers disagree on the scope");
        }
        let folder_in_scope = base.in_scope(scope, note.parent);
        let from_path = base.path(note.parent);
        let scope_path = base.path(scope);

        // paths
        for target in world
            .files
            .iter()
            .filter(|f| world.visible(USERS[author], f.id))
        {
            let target_path = base.path(target.id);
            let in_scope = base.in_scope(scope, target.id);
            let rel = relative_path(&from_path, &target_path);
            let anchored = format!("/{}", target_path.strip_prefix(&scope_path).unwrap_or(""));
            let escape = format!("{}{rel}", "../".repeat(MAX_TREE_DEPTH + 1));
            for (url, round_trips) in [
                (&rel, folder_in_scope && in_scope),
                (&anchored, in_scope && target.id != scope),
                (&target_path, false),
                (&escape, false),
            ] {
                let answer = resolve(base, url, note.id);
                if readers.iter().any(|v| resolve(v, url, note.id) != answer) {
                    return Err("path: readers disagree");
                }
                if answer.is_some_and(|id| !base.in_scope(scope, id)) {
                    return Err("path: resolved outside the scope");
                }
                if answer.is_some_and(|id| !base.get_by_id(id).unwrap().is_document()) {
                    return Err("path: resolved to a folder");
                }
                if round_trips && target.is_document() && answer != Some(target.id) {
                    return Err("path: round trip");
                }
            }
            if resolve(base, &escape, note.id).is_some() {
                return Err("path: excess `..` resolved");
            }
        }

        // lb://
        for target in &docs {
            let url = format!("lb://{}", target.id);
            for (user, view) in USERS.iter().zip(&views) {
                if !world.visible(user, note.id) {
                    continue;
                }
                let answer = resolve(view, &url, note.id);
                if answer.is_some() && !world.visible(&note.owner, target.id) {
                    return Err("lb://: resolved past the author's reach");
                }
                let expected = target.owner == note.owner && world.visible(user, target.id);
                if expected && answer != Some(target.id) {
                    return Err("lb://: the author's own document must resolve");
                }
            }
        }

        // wiki titles; the scope's top goes by a different name for each reader
        let in_scope: Vec<&File> = base
            .iter_files()
            .filter(|d| d.is_document() && d.id != scope && base.in_scope(scope, d.id))
            .collect();
        for target in &docs {
            let stem = strip_ext(&target.name).to_string();
            let trailing = format!("{}/{stem}", world.get(target.parent).name);
            for title in [&stem, &target.name, &trailing] {
                let answer = base.resolve_wikilink(title, note.id);
                if readers
                    .iter()
                    .any(|v| v.resolve_wikilink(title, note.id) != answer)
                {
                    return Err("wiki: readers disagree");
                }
                let Some(id) = answer else { continue };
                if !in_scope.iter().any(|d| d.id == id) {
                    return Err("wiki: resolved outside the scope");
                }
                let name = &base.get_by_id(id).unwrap().name;
                let last = title.rsplit('/').next().unwrap();
                if !name.eq_ignore_ascii_case(last) && !strip_ext(name).eq_ignore_ascii_case(last) {
                    return Err("wiki: resolved file doesn't match the title");
                }
            }
        }
        for target in &in_scope {
            let stem = strip_ext(&target.name);
            let count =
                |pred: &dyn Fn(&str) -> bool| in_scope.iter().filter(|d| pred(&d.name)).count();
            if count(&|n| strip_ext(n).eq_ignore_ascii_case(stem)) == 1
                && base.resolve_wikilink(stem, note.id) != Some(target.id)
            {
                return Err("wiki: a stem unique in the scope must resolve");
            }
            if count(&|n| n.eq_ignore_ascii_case(&target.name)) == 1
                && base.resolve_wikilink(&target.name, note.id) != Some(target.id)
            {
                return Err("wiki: a name unique in the scope must resolve");
            }
        }

        // what completions and paste write resolves back, for every reader
        for target in base.iter_files().filter(|f| f.is_document()) {
            let dest = base.link_destination(target.id, note.id);
            if resolve(base, &dest, note.id) != Some(target.id) {
                return Err("written destination: round trip");
            }
            let Some(title) = base.wikilink_title(target.id, note.id) else { continue };
            if readers
                .iter()
                .any(|v| v.resolve_wikilink(&title, note.id) != Some(target.id))
            {
                return Err("written wiki title: round trip");
            }
        }

        // siblings of the note sharing a stem are equally near
        let siblings: Vec<&&File> = in_scope
            .iter()
            .filter(|d| d.parent == note.parent)
            .collect();
        for d in &siblings {
            let stem = strip_ext(&d.name);
            let sharing = siblings
                .iter()
                .filter(|x| strip_ext(&x.name).eq_ignore_ascii_case(stem))
                .count();
            let exact = in_scope.iter().any(|x| x.name.eq_ignore_ascii_case(stem));
            if sharing >= 2 && !exact && base.resolve_wikilink(stem, note.id).is_some() {
                return Err("wiki: an ambiguous stem must not resolve");
            }
        }
    }
    Ok(())
}

/// Runs `check` across 2048 seeded buffers. On failure, delta-debugs the
/// input and panics with the shrunken buffer and its reconstructed world.
#[test]
fn link_resolution() {
    for seed in 0..2048u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut buf = vec![0u8; 128];
        rng.fill(&mut buf[..]);
        if let Err(reason) = check(&buf) {
            let shrunk = shrink(buf, |b| check(b).is_err());
            let mut src = ByteSource::new(&shrunk);
            panic!(
                "seed {seed} {reason}\nshrunk ({} bytes): {shrunk:?}\nfiles:\n{:#?}",
                shrunk.len(),
                world(&mut src).files,
            );
        }
    }
}
