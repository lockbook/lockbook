//! Property tests for link resolution across the participants of a note.
//!
//! Alice owns a tree and shares one file in it with Bob and another with
//! Carol; the two may nest. Bob owns a tree too and may share a file in it
//! with Alice. Each participant's view is built the way lb-rs lists it: their
//! own files, plus each share they hold, either pending (topped by a parent
//! they can't see) or placed in a folder of theirs that isn't shared (lb-rs
//! refuses links in shared folders), possibly renamed. Names come from one
//! small pool, so the same names turn up in every tree.
//!
//! For every note of Alice's that others read and every destination any
//! view could write toward any document, every view holding the note must
//! agree:
//!
//! - **Same note, same answer.** A link that resolves in the scope resolves
//!   there to the same document, as [`Reach::Scope`], in every view that
//!   resolves it at all; anything past the scope is [`Reach::Reader`].
//! - **Placement is irrelevant.** Pending and placed views of one participant
//!   are two of those views.
//! - **Reader-dependent paths are marked.** An absolute path in a shared note
//!   is [`Reach::Reader`] wherever it resolves.
//! - **What one holder writes, every holder reads.** The title or destination
//!   the cache writes toward a document in the scope, in any view, names
//!   that document in the scope in every view.
//!
//! Confirmed via fault injection: the scope taken as the reader's whole tree,
//! `..` allowed past the scope's top, and absolute paths in shared notes left
//! unmarked each fail.

use std::collections::{BTreeSet, HashMap};

use lb_rs::Uuid;
use lb_rs::model::file::{File, Share, ShareMode};
use lb_rs::model::file_metadata::FileType;
use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::file_cache::{FileCache, FilesExt, Reach, relative_path, strip_ext};
use crate::file_cache_prop_tests::{POOL, file, fill_subtree};
use crate::test_utils::byte_source::ByteSource;
use crate::test_utils::shrink::shrink;

const ALICE: &str = "alice";
const BOB: &str = "bob";
const CAROL: &str = "carol";

/// Every file in the world at its true place, shares included.
struct World {
    files: Vec<File>,
    roots: HashMap<&'static str, Uuid>,
}

impl World {
    fn get(&self, id: Uuid) -> &File {
        self.files.iter().find(|f| f.id == id).unwrap()
    }

    fn under(&self, top: Uuid) -> Vec<File> {
        self.files
            .iter()
            .filter(|f| self.files.ancestors(f.id).contains(&top))
            .cloned()
            .collect()
    }

    /// `user`'s view: their own tree, and the share they hold pending or
    /// placed.
    fn view(&self, user: &'static str, placed: bool, src: &mut ByteSource) -> FileCache {
        let root = self.get(self.roots[user]).clone();
        let own: Vec<File> = self
            .files
            .iter()
            .filter(|f| f.owner == user)
            .cloned()
            .collect();
        let mut shared = vec![];
        let held = self
            .files
            .iter()
            .find(|f| f.owner != user && f.shares.iter().any(|s| s.shared_with == user));
        if let Some(top) = held {
            let mut top = top.clone();
            let free: Vec<&File> = own
                .iter()
                .filter(|f| f.is_folder())
                .filter(|f| {
                    let mut chain = own.ancestors(f.id);
                    chain.push(f.id);
                    chain
                        .iter()
                        .all(|id| own.get_by_id(*id).unwrap().shares.is_empty())
                })
                .collect();
            if placed {
                top.parent = free[src.draw(free.len())].id;
                let name = POOL[src.draw(POOL.len())];
                if src.draw(2) == 1 && !own.iter().any(|f| f.parent == top.parent && f.name == name)
                {
                    top.name = name.into();
                }
                if own
                    .iter()
                    .any(|f| f.parent == top.parent && f.name == top.name)
                {
                    top.name = format!("{} (shared)", top.name);
                }
            }
            shared.extend(self.under(top.id));
            shared.push(top);
        }
        FileCache::from_owned_and_shared(root, own, shared)
    }
}

fn share(file: &mut File, by: &str, with: &str) {
    file.shares.push(Share {
        mode: ShareMode::Write,
        shared_by: by.into(),
        shared_with: with.into(),
    });
}

fn world(src: &mut ByteSource) -> World {
    let mut files = vec![];
    let mut roots = HashMap::new();
    for user in [ALICE, BOB, CAROL] {
        let root = Uuid::new_v4();
        let mut tree = vec![file(root, root, "root", FileType::Folder)];
        if user != CAROL {
            fill_subtree(&mut tree, src, root);
        }
        for f in &mut tree {
            f.owner = user.into();
        }
        roots.insert(user, root);
        files.extend(tree);
    }

    let pick = |files: &[File], owner: &str, src: &mut ByteSource| {
        let candidates: Vec<usize> = (0..files.len())
            .filter(|&i| files[i].owner == owner && !files[i].is_root())
            .collect();
        (!candidates.is_empty()).then(|| candidates[src.draw(candidates.len())])
    };
    if let Some(i) = pick(&files, ALICE, src) {
        share(&mut files[i], ALICE, BOB);
    }
    if let Some(i) = pick(&files, ALICE, src) {
        share(&mut files[i], ALICE, CAROL);
    }
    if src.draw(2) == 1 {
        if let Some(i) = pick(&files, BOB, src) {
            share(&mut files[i], BOB, ALICE);
        }
    }
    World { files, roots }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Form {
    Relative,
    Absolute,
    Id,
    Wiki,
}

/// Every destination some view could write in `note` toward a document it
/// sees.
fn destinations(views: &[(String, FileCache)], note: Uuid) -> BTreeSet<(Form, String)> {
    let mut out = BTreeSet::new();
    for (_, view) in views {
        let Some(source) = view.get_by_id(note) else { continue };
        let from_path = view.path(source.parent);
        for f in view.iter_files().filter(|f| f.is_document()) {
            let path = view.path(f.id);
            let rel = relative_path(&from_path, &path);
            out.insert((Form::Relative, rel.clone()));
            out.insert((Form::Wiki, rel.clone()));
            if let Some((dir, _)) = rel.rsplit_once('/') {
                out.insert((Form::Wiki, format!("{dir}/{}", strip_ext(&f.name))));
            }
            out.insert((Form::Wiki, f.name.clone()));
            out.insert((Form::Wiki, strip_ext(&f.name).to_string()));
            out.insert((Form::Id, format!("lb://{}", f.id)));
            out.insert((Form::Id, format!("https://app.lockbook.net/open/{}", f.id)));
            if path.starts_with('/') {
                out.insert((Form::Absolute, path));
            }
        }
    }
    out
}

fn check(buf: &[u8]) -> Result<(), String> {
    let mut src = ByteSource::new(buf);
    let world = world(&mut src);
    let mut views = vec![];
    for user in [ALICE, BOB, CAROL] {
        for placed in [false, true] {
            views.push((format!("{user} placed={placed}"), world.view(user, placed, &mut src)));
        }
    }

    let notes = world
        .files
        .iter()
        .filter(|f| f.owner == ALICE && f.is_document());
    for note in notes {
        let holders: Vec<&(String, FileCache)> = views
            .iter()
            .filter(|(_, v)| v.get_by_id(note.id).is_some())
            .collect();
        // Alice's own placements change her unshared notes, which only she reads
        if holders.iter().all(|(name, _)| name.starts_with(ALICE)) {
            continue;
        }
        for (form, dest) in destinations(&views, note.id) {
            let fail = |what: &str, answers: Vec<String>| {
                Err(format!("{what}: {form:?} {dest:?} in {:?}\n{}", note.name, answers.join("\n")))
            };
            let located: Vec<Option<(Uuid, Reach)>> = holders
                .iter()
                .map(|(_, v)| v.locate_link(&dest, note.id))
                .collect();
            let answers = || {
                holders
                    .iter()
                    .zip(&located)
                    .map(|((name, v), l)| {
                        let wiki = v.resolve_wikilink(&dest, note.id);
                        let names = |id: Uuid| v.path(id);
                        format!(
                            "  {name}: link {:?} wiki {:?}",
                            l.map(|(id, r)| (names(id), r)),
                            wiki.map(names)
                        )
                    })
                    .collect::<Vec<_>>()
            };

            match form {
                Form::Wiki | Form::Relative => {
                    let located: Vec<Option<(Uuid, Reach)>> = match form {
                        Form::Wiki => holders
                            .iter()
                            .map(|(_, v)| v.locate_wikilink(&dest, note.id))
                            .collect(),
                        _ => located.clone(),
                    };
                    let scoped: Vec<_> = located
                        .iter()
                        .map(|l| l.filter(|(_, r)| *r == Reach::Scope))
                        .collect();
                    if scoped.iter().any(|l| *l != scoped[0]) {
                        return fail("paths disagree in scope", answers());
                    }
                    if scoped[0].is_some() && located.iter().any(|l| l != &scoped[0]) {
                        return fail("in scope for some, past it for others", answers());
                    }
                }
                Form::Id => {
                    let mut reaches: Vec<Reach> = located.iter().flatten().map(|l| l.1).collect();
                    reaches.dedup();
                    if reaches.len() > 1 {
                        return fail("id link reaches disagree", answers());
                    }
                    if reaches.contains(&Reach::Scope) && located.iter().any(|l| l.is_none()) {
                        return fail("in scope for some, broken for others", answers());
                    }
                }
                Form::Absolute => {
                    for ((_, v), l) in holders.iter().zip(&located) {
                        let shared = v.scope_top(note.id) != v.root().id;
                        if shared && matches!(l, Some((_, r)) if *r != Reach::Reader) {
                            return fail("unmarked absolute path in a shared note", answers());
                        }
                    }
                }
            }
        }

        for (writer, v) in &holders {
            let scope = v.scope_top(note.id);
            let targets = v
                .iter_files()
                .filter(|f| f.is_document() && f.id != note.id && f.id != scope)
                .filter(|f| v.in_scope(scope, f.id));
            for target in targets {
                let written = [
                    v.wikilink_title(target.id, note.id)
                        .map(|t| (Form::Wiki, t)),
                    v.link_destination(target.id, note.id)
                        .map(|d| (Form::Relative, d)),
                ];
                for (form, dest) in written.into_iter().flatten() {
                    for (reader, w) in &holders {
                        let read = match form {
                            Form::Wiki => w.locate_wikilink(&dest, note.id),
                            _ => w.locate_link(&dest, note.id),
                        };
                        if read != Some((target.id, Reach::Scope)) {
                            return Err(format!(
                                "{writer} wrote {form:?} {dest:?} for {} in {:?}; {reader} reads {:?}",
                                v.path(target.id),
                                note.name,
                                read.map(|(id, r)| (w.path(id), r))
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn same_note_same_answer() {
    for seed in 0..1024u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut buf = vec![0u8; 128];
        rng.fill(&mut buf[..]);
        if check(&buf).is_err() {
            let shrunk = shrink(buf, |b| check(b).is_err());
            let reason = check(&shrunk).unwrap_err();
            let world = world(&mut ByteSource::new(&shrunk));
            let tree: Vec<String> = world
                .files
                .iter()
                .map(|f| {
                    let shares: Vec<&str> =
                        f.shares.iter().map(|s| s.shared_with.as_str()).collect();
                    format!("  {} {} {:?}", f.owner, world.files.path(f.id), shares)
                })
                .collect();
            panic!("seed {seed}: {reason}\nshrunk {shrunk:?}\nworld:\n{}", tree.join("\n"));
        }
    }
}
