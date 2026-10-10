use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::mem;

use lb_rs::Uuid;
use lb_rs::model::file::File;

use super::{Link, LinkKind};
use crate::file_cache::{FilesExt, ResolvedLink, UuidMap};

/// A link in a note, with the file it reaches.
#[derive(Clone, Debug)]
pub struct Indexed {
    pub link: Link,
    /// The file the link reaches as the tree stands.
    pub target: Option<Uuid>,
    /// The file it reached when written or last settled. Differs from
    /// `target` once a rename, move, share, or same-named file elsewhere has
    /// taken the link from it. A link first seen broken means the file it
    /// would reach were nothing shared.
    pub meant: Option<Uuid>,
    /// How much of the destination names `meant`; the rest is a fragment.
    pub named: usize,
}

struct Note {
    /// The note's `last_modified` as of the text its links were read from.
    version: u64,
    links: Vec<Indexed>,
}

/// Every link in every note, by the note it is written in and by the file it
/// reaches.
#[derive(Default)]
pub struct LinkIndex {
    notes: UuidMap<Note>,
    /// Notes with a link reaching each file, ascending.
    linkers: UuidMap<Vec<Uuid>>,
    /// The [`shape`] of the tree the links were last resolved in.
    shape: u64,
}

/// What [`LinkIndex::refresh`] found gone.
#[derive(Default)]
pub struct Gone {
    /// A note and a file it linked to that no longer exists.
    pub lost: Vec<(Uuid, Uuid)>,
    /// Files that notes no longer in the tree linked to.
    pub dropped: Vec<Uuid>,
}

impl LinkIndex {
    /// The links written in `note`, in order.
    pub fn outbound(&self, note: Uuid) -> &[Indexed] {
        self.notes.get(&note).map_or(&[], |n| &n.links)
    }

    /// Every note read, with the links written in it.
    pub fn notes(&self) -> impl Iterator<Item = (Uuid, &[Indexed])> {
        self.notes.iter().map(|(note, n)| (*note, &n.links[..]))
    }

    /// The notes with a link that reaches `file`.
    pub fn linkers(&self, file: Uuid) -> &[Uuid] {
        self.linkers.get(&file).map_or(&[], |notes| notes)
    }

    /// Each link that reaches `file`, with the note it is written in.
    pub fn inbound(&self, file: Uuid) -> impl Iterator<Item = (Uuid, &Indexed)> {
        self.linkers(file).iter().flat_map(move |note| {
            let links = self.outbound(*note).iter();
            links
                .filter(move |l| l.target == Some(file))
                .map(|l| (*note, l))
        })
    }

    /// The `last_modified` of `note` as of the text its links were read from.
    pub fn version(&self, note: Uuid) -> Option<u64> {
        self.notes.get(&note).map(|n| n.version)
    }

    /// Links that don't reach the file they are meant to, with the note each
    /// is written in.
    pub fn strays(&self) -> impl Iterator<Item = (Uuid, &Indexed)> {
        self.notes().flat_map(|(note, links)| {
            let strays = links
                .iter()
                .filter(|l| l.meant.is_some() && l.meant != l.target);
            strays.map(move |l| (note, l))
        })
    }

    /// Records the links read from `note`'s text. A link written as one
    /// already there still means what that one meant, except that each link
    /// the note has gained to a file accounts for one that strayed from it.
    /// Returns the files the note reached before and doesn't now.
    pub fn set<F: FilesExt + ?Sized>(
        &mut self, files: &F, note: Uuid, version: u64, links: Vec<Link>,
    ) -> Vec<Uuid> {
        let mut old = self.notes.remove(&note).map_or(vec![], |n| n.links);
        let before = reached(&old);
        for link in &mut old {
            link.target = reach(files, note, &link.link).map(|(file, _)| file);
        }
        let reaches: Vec<_> = links.iter().map(|link| reach(files, note, link)).collect();

        // how many links, of those that strayed from each file, still mean it
        let mut strays: UuidMap<usize> = Default::default();
        let strayed = old.iter().filter(|l| l.meant != l.target);
        for meant in strayed.filter_map(|l| l.meant) {
            *strays.entry(meant).or_default() += 1;
        }
        for (file, strays) in &mut strays {
            let had = old.iter().filter(|l| l.target == Some(*file)).count();
            let reaches = reaches.iter().flatten();
            let has = reaches.filter(|(reached, _)| reached == file).count();
            *strays = strays.saturating_sub(has.saturating_sub(had));
        }

        let links = links.into_iter().zip(reaches);
        let links: Vec<Indexed> = links
            .map(|(link, reached)| {
                let target = reached.map(|(file, _)| file);
                let same = |o: &&Indexed| o.link.kind == link.kind && o.link.dest == link.dest;
                let carried = old.iter().find(same).filter(|old| match old.meant {
                    Some(meant) if old.meant != target => {
                        let left = strays.entry(meant).or_default();
                        mem::replace(left, left.saturating_sub(1)) > 0
                    }
                    meant => meant.is_none() && target.is_none(),
                });
                let (meant, named) = match carried {
                    Some(old) => (old.meant, old.named),
                    None => match reached.or_else(|| reach_beyond(files, note, &link)) {
                        Some((file, named)) => (Some(file), named),
                        None => (None, link.dest.len()),
                    },
                };
                Indexed { link, target, meant, named }
            })
            .collect();

        let after = reached(&links);
        for file in &before {
            self.unlink(note, *file);
        }
        for file in &after {
            let notes = self.linkers.entry(*file).or_default();
            if let Err(at) = notes.binary_search(&note) {
                notes.insert(at, note);
            }
        }
        self.notes.insert(note, Note { version, links });
        before.into_iter().filter(|f| !after.contains(f)).collect()
    }

    /// Resolves every link again if the tree has changed shape, and forgets
    /// notes that are no longer in it.
    pub fn refresh<F: FilesExt + ?Sized>(&mut self, files: &F) -> Gone {
        let mut gone = Gone::default();
        let shape = shape(files);
        if mem::replace(&mut self.shape, shape) == shape {
            return gone;
        }
        let exists = |id: Uuid| files.get_by_id(id).is_some_and(|f| f.is_document());
        self.notes.retain(|note, entry| {
            if exists(*note) {
                return true;
            }
            gone.dropped
                .extend(entry.links.iter().filter_map(|l| l.target));
            false
        });
        for (note, entry) in &mut self.notes {
            for link in &mut entry.links {
                let reached = reach(files, *note, &link.link);
                link.target = reached.map(|(file, _)| file);
                if let Some(meant) = link.meant.filter(|meant| !exists(*meant)) {
                    gone.lost.push((*note, meant));
                    link.meant = None;
                }
                if let Some((file, named)) = reached.filter(|r| link.meant.is_none_or(|m| m == r.0))
                {
                    (link.meant, link.named) = (Some(file), named);
                }
            }
        }

        self.linkers.clear();
        for (note, entry) in &self.notes {
            for file in entry.links.iter().filter_map(|l| l.target) {
                self.linkers.entry(file).or_default().push(*note);
            }
        }
        for notes in self.linkers.values_mut() {
            notes.sort();
            notes.dedup();
        }
        gone.lost.sort();
        gone.lost.dedup();
        gone.dropped.retain(|file| exists(*file));
        gone
    }

    /// Takes the links in `note` written as `dest` to mean what they reach
    /// now.
    pub fn settle(&mut self, note: Uuid, dest: &str) {
        if let Some(entry) = self.notes.get_mut(&note) {
            for link in entry.links.iter_mut().filter(|l| l.link.dest == dest) {
                link.meant = link.target;
            }
        }
    }

    fn unlink(&mut self, note: Uuid, file: Uuid) {
        if let Some(notes) = self.linkers.get_mut(&file) {
            notes.retain(|n| *n != note);
            if notes.is_empty() {
                self.linkers.remove(&file);
            }
        }
    }
}

/// The files `links` reach, ascending.
fn reached(links: &[Indexed]) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = links.iter().filter_map(|l| l.target).collect();
    ids.sort();
    ids.dedup();
    ids
}

/// A digest of what where links lead rests on: which files there are, where,
/// named what, and shared how. A write to a document leaves it as it was.
pub(super) fn shape<F: FilesExt + ?Sized>(files: &F) -> u64 {
    let digest = |f: &File| {
        let mut hasher = DefaultHasher::new();
        (f.id, f.parent, &f.name, f.file_type, &f.owner, &f.shares).hash(&mut hasher);
        hasher.finish()
    };
    files.iter_files().fold(0, |shape, f| shape ^ digest(f))
}

/// The file `link` reaches from `note`, and how much of its destination names
/// that file; the rest is a fragment.
pub(super) fn reach<F: FilesExt + ?Sized>(
    files: &F, note: Uuid, link: &Link,
) -> Option<(Uuid, usize)> {
    let dest = &link.dest;
    let file = |dest: &str| match files.resolve_link(dest, note)? {
        ResolvedLink::File(id) => Some(id),
        ResolvedLink::External(_) => None,
    };
    match link.kind {
        LinkKind::Wiki => match files.wikilink_split(dest, note, files.scope_top(note)) {
            (matches, named) if matches.len() == 1 => Some((matches[0].id, named)),
            _ => None,
        },
        LinkKind::Link | LinkKind::Image => file(dest).map(|id| (id, named(dest, id, file))),
    }
}

/// [`reach`] were the note's scope its owner's whole tree.
fn reach_beyond<F: FilesExt + ?Sized>(files: &F, note: Uuid, link: &Link) -> Option<(Uuid, usize)> {
    let (dest, wiki) = (&link.dest, link.kind == LinkKind::Wiki);
    let id = files.beyond_scope(dest, wiki, note)?;
    let named = match wiki {
        true => files.wikilink_split(dest, note, files.tree_root(note)).1,
        false => named(dest, id, |dest| files.beyond_scope(dest, false, note)),
    };
    Some((id, named))
}

/// How much of a destination that reaches `file` names it: all of it when a
/// name has the `#`, and what is before the `#` otherwise.
fn named(dest: &str, file: Uuid, reach: impl Fn(&str) -> Option<Uuid>) -> usize {
    match dest.find('#') {
        Some(at) if reach(&dest[..at]) == Some(file) => at,
        _ => dest.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::extract;
    use crate::test_utils::files::{at, tree};

    fn project() -> Vec<File> {
        tree("alice", &["/budget.md", "/project/plan.md", "/project/notes.md", "/project/design/"])
    }

    fn index(files: &Vec<File>, notes: &[(&str, &str)]) -> LinkIndex {
        let mut index = LinkIndex::default();
        for (path, text) in notes {
            index.set(files, at(files, path), 0, extract(text));
        }
        index
    }

    fn rename(files: &mut [File], path: &str, name: &str) {
        let id = at(files, path);
        files.iter_mut().find(|f| f.id == id).unwrap().name = name.into();
    }

    #[test]
    fn reads_both_ways() {
        let files = project();
        let text = "[plan](plan.md), [[budget]], ![](plan.md), https://lockbook.net, [[nowhere]]";
        let index = index(&files, &[("/project/notes.md", text), ("/budget.md", "[[plan]]")]);
        let (notes, plan, budget) = (
            at(&files, "/project/notes.md"),
            at(&files, "/project/plan.md"),
            at(&files, "/budget.md"),
        );

        let reached: Vec<_> = index.outbound(notes).iter().map(|l| l.target).collect();
        assert_eq!(reached, [Some(plan), Some(budget), Some(plan), None, None]);
        assert_eq!(index.linkers(plan), [budget, notes]);
        assert_eq!(index.linkers(budget), [notes]);
        assert_eq!(
            index
                .inbound(plan)
                .filter(|(note, _)| *note == notes)
                .count(),
            2
        );
        assert_eq!(index.strays().count(), 0);
    }

    #[test]
    fn a_rewritten_note_reports_what_it_dropped() {
        let files = project();
        let mut index = index(&files, &[("/project/notes.md", "[[plan]] [[budget]]")]);
        let (notes, plan, budget) = (
            at(&files, "/project/notes.md"),
            at(&files, "/project/plan.md"),
            at(&files, "/budget.md"),
        );

        assert_eq!(index.set(&files, notes, 1, extract("[[budget]] and more")), [plan]);
        assert!(index.linkers(plan).is_empty());
        assert_eq!(index.linkers(budget), [notes]);
        assert_eq!(index.version(notes), Some(1));
    }

    #[test]
    fn a_renamed_file_leaves_strays_that_remember_it() {
        let mut files = project();
        let mut index = index(&files, &[("/project/notes.md", "[plan](plan.md) [[plan]]")]);
        let (notes, plan) = (at(&files, "/project/notes.md"), at(&files, "/project/plan.md"));

        rename(&mut files, "/project/plan.md", "roadmap.md");
        let gone = index.refresh(&files);
        assert!(gone.lost.is_empty() && gone.dropped.is_empty());
        let strays: Vec<_> = index.strays().collect();
        assert_eq!(strays.len(), 2);
        assert!(
            strays
                .iter()
                .all(|(note, l)| *note == notes && l.meant == Some(plan))
        );
        assert!(index.linkers(plan).is_empty());

        // unrelated edits to the note keep what its links meant
        index.set(&files, notes, 1, extract("edited [plan](plan.md) [[plan]]"));
        assert_eq!(index.strays().count(), 2);
        // rewritten links are new links
        index.set(&files, notes, 2, extract("[plan](roadmap.md) [[plan]]"));
        assert_eq!(index.strays().count(), 1);
        index.set(&files, notes, 3, extract("[plan](roadmap.md) [[roadmap]]"));
        assert_eq!(index.strays().count(), 0);
        assert_eq!(index.linkers(plan), [notes]);
    }

    /// A link rewritten into the form another link strayed in is not that
    /// stray: the other was rewritten to reach its file.
    #[test]
    fn a_rewritten_link_isnt_the_stray_it_is_now_written_as() {
        let mut files = tree("alice", &["/p/todo.md", "/p/b.md", "/p/a/b.md"]);
        let todo = at(&files, "/p/todo.md");
        let mut index = index(&files, &[("/p/todo.md", "[x](a/b.md) [y](b.md)")]);
        files.iter_mut().find(|f| f.id == todo).unwrap().parent = at(&files, "/p/a");
        index.refresh(&files);
        assert_eq!(index.strays().count(), 2);

        index.set(&files, todo, 1, extract("[x](b.md) [y](../b.md)"));
        assert_eq!(index.strays().count(), 0);
    }

    #[test]
    fn a_nearer_namesake_captures_a_wikilink() {
        let mut files = project();
        let mut index = index(&files, &[("/project/notes.md", "[[budget]]")]);
        let budget = at(&files, "/budget.md");

        files.extend(tree("alice", &["/project/budget.md"]).pop().map(|mut f| {
            f.id = Uuid::from_u128(99);
            f.parent = at(&files, "/project");
            f
        }));
        index.refresh(&files);
        let strays: Vec<_> = index.strays().collect();
        assert_eq!(strays.len(), 1);
        assert_eq!(
            (strays[0].1.meant, strays[0].1.target),
            (Some(budget), Some(Uuid::from_u128(99)))
        );

        // taking the capture as meant
        index.settle(at(&files, "/project/notes.md"), "budget");
        assert_eq!(index.strays().count(), 0);
    }

    #[test]
    fn a_link_out_of_a_share_is_a_stray_from_the_start() {
        let files = tree("alice", &["/budget.md", "/project/ @bob", "/project/notes.md"]);
        let (notes, budget) = (at(&files, "/project/notes.md"), at(&files, "/budget.md"));
        let text = "[b](../budget.md) [b](/budget.md) [[budget]] [gone](../nowhere.md)";
        let index = index(&files, &[("/project/notes.md", text)]);
        let strays: Vec<_> = index.strays().map(|(_, l)| (l.target, l.meant)).collect();
        assert_eq!(strays, [(None, Some(budget)); 3]);
        assert_eq!(index.outbound(notes)[3].meant, None);
    }

    #[test]
    fn only_a_change_of_shape_is_a_change_to_resolve() {
        let files = tree("alice", &["/a/x.md @bob", "/b.md"]);
        let changed = |change: &dyn Fn(&mut Vec<File>)| {
            let mut changed = files.clone();
            change(&mut changed);
            shape(&changed) != shape(&files)
        };
        assert!(!changed(&|f| (f[2].last_modified, f[2].size_bytes) = (7, 7)));
        assert!(!changed(&|f| f.reverse()));
        assert!(changed(&|f| f[2].name = "y.md".into()));
        assert!(changed(&|f| f[2].parent = f[0].id));
        assert!(changed(&|f| f[2].shares.clear()));
        assert!(changed(&|f| drop(f.pop())));
        assert!(changed(&|f| (f[2].name, f[3].name) = (f[3].name.clone(), f[2].name.clone())));
    }

    #[test]
    fn a_fragment_is_what_follows_the_name() {
        let paths = ["/C# notes.md", "/plan.md", "/notes.md", "/shared/ @bob", "/shared/log.md"];
        let files = tree("alice", &paths);
        let text = "[[C# notes]] [[C# notes#Generics]] [[plan#Goals#More]] [p](plan.md#goals) \
                    [c](C%23%20notes.md) [[#here]] [[nowhere#else]]";
        let index = index(&files, &[("/notes.md", text), ("/shared/log.md", "[[plan#Goals]]")]);
        let fragments = |path: &str| -> Vec<String> {
            let links = index.outbound(at(&files, path)).iter();
            links.map(|l| l.link.dest[l.named..].to_string()).collect()
        };
        assert_eq!(
            fragments("/notes.md"),
            ["", "#Generics", "#Goals#More", "#goals", "", "#here", ""]
        );
        // of what it meant before the share, too
        assert_eq!(fragments("/shared/log.md"), ["#Goals"]);
    }

    #[test]
    fn a_link_that_passes_to_a_namesake_still_lost_its_file() {
        let mut files = tree("alice", &["/a/Meeting.md", "/a/notes.md", "/b/Meeting.md"]);
        let (notes, near, far) =
            (at(&files, "/a/notes.md"), at(&files, "/a/Meeting.md"), at(&files, "/b/Meeting.md"));
        let mut index = index(&files, &[("/a/notes.md", "[[Meeting]]")]);
        assert_eq!(index.outbound(notes)[0].target, Some(near));

        files.retain(|f| f.id != near);
        assert_eq!(index.refresh(&files).lost, [(notes, near)]);
        assert_eq!(index.outbound(notes)[0].target, Some(far));
        assert_eq!(index.strays().count(), 0);
    }

    #[test]
    fn deleted_files_are_lost_and_deleted_notes_drop_what_they_reached() {
        let mut files = project();
        let mut index = index(
            &files,
            &[("/project/notes.md", "[[plan]] [[budget]]"), ("/project/plan.md", "[[budget]]")],
        );
        let (notes, plan, budget) = (
            at(&files, "/project/notes.md"),
            at(&files, "/project/plan.md"),
            at(&files, "/budget.md"),
        );

        files.retain(|f| f.id != plan);
        let gone = index.refresh(&files);
        assert_eq!(gone.lost, [(notes, plan)]);
        assert_eq!(gone.dropped, [budget]);
        assert_eq!(index.strays().count(), 0);
        assert_eq!(index.linkers(budget), [notes]);
    }
}
