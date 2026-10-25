//! What a change to the tree does to links, worked out before it's made:
//! the edits that keep links reaching their files, which the user accepts or
//! declines, and the links no edit here can keep.

use std::collections::HashSet;
use std::sync::Arc;

use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::errors::LbResult;

use crate::file_cache::{
    Change, FileCache, FilesExt, Reach, encode_link_path, relative_path, split_fragment,
    title_matches, wiki_fragment,
};
use crate::links::{LinkKind, NoteLink, written_dests};
use crate::note_index::{Backlink, locate};
use crate::resolvers::link::can_edit;

/// A link's new destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mend {
    pub note: Uuid,
    pub kind: LinkKind,
    /// The destination as written now, and as it would be. An id link is
    /// planned as `lb://` and written as the external URL.
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Edits that keep links reaching their files.
    pub mends: Vec<Mend>,
    /// Links the change breaks that no edit here keeps: in notes the user
    /// can't write, or with no form that reaches the file from there.
    pub stuck: Vec<Backlink>,
    /// Made before every note was first read, so links in the unread ones
    /// aren't counted.
    pub checking: bool,
    /// For a share: the files those links reach that the user owns and could
    /// share too, which the mends then link to by id.
    pub share_too: Vec<Uuid>,
}

impl Plan {
    /// The notes the mends edit.
    pub fn mended_notes(&self) -> Vec<Uuid> {
        distinct(self.mends.iter().map(|m| m.note))
    }

    pub fn stuck_notes(&self) -> Vec<Uuid> {
        distinct(self.stuck.iter().map(|b| b.note))
    }

    /// Nothing to say: no edits, nothing breaking, and every note read.
    pub fn is_empty(&self) -> bool {
        self.mends.is_empty() && self.stuck.is_empty() && !self.checking
    }
}

fn distinct(ids: impl Iterator<Item = Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.filter(|id| seen.insert(*id)).collect()
}

/// [`plan`], kept until the notes or the tree change: sheets ask each
/// frame.
pub fn planned(files: &FileCache, change: &Change) -> Arc<Plan> {
    let mut planned = files.notes.planned.lock().unwrap();
    match &*planned {
        Some((c, plan)) if c == change => plan.clone(),
        _ => {
            let plan = Arc::new(plan(files, change));
            *planned = Some((change.clone(), plan.clone()));
            plan
        }
    }
}

/// What `change` would do to the links `files` has indexed. A link whose
/// file would change is mended to reach the file it reaches now, in the form
/// it's written in; a delete mends nothing and lists what breaks.
pub fn plan(files: &FileCache, change: &Change) -> Plan {
    let mut plan = changes(files, change);
    plan.checking = !files.notes.is_ready();
    plan
}

fn changes(files: &FileCache, change: &Change) -> Plan {
    let after = files.after(change);
    if let Change::Share { id, .. } = change {
        return share_plan(files, &after, *id);
    }
    let moved: Vec<Uuid> = match change {
        Change::Rename { id, .. } | Change::Share { id, .. } => vec![*id],
        Change::Move { ids, .. } | Change::Delete { ids } => ids.clone(),
        Change::Create { .. } => vec![Change::CREATED],
    };
    let affected: HashSet<Uuid> = moved
        .iter()
        .flat_map(|id| {
            let under: Vec<Uuid> = files.descendents(*id).iter().map(|f| f.id).collect();
            std::iter::once(*id).chain(under)
        })
        .collect();
    // names a wiki title might newly match
    let names: Vec<&str> = affected
        .iter()
        .filter_map(|id| after.get_by_id(*id))
        .filter(|f| f.is_document())
        .map(|f| f.name.as_str())
        .collect();

    // the links that could change: those reaching what changes, those in
    // what moves, and wiki titles a new name could take
    let index = &files.notes;
    let inbound = index.inbound(files);
    let mut candidates: HashSet<(Uuid, usize)> = affected
        .iter()
        .filter_map(|id| inbound.get(id))
        .flatten()
        .copied()
        .collect();
    for (note, text) in index.notes() {
        for (i, link) in text.links.iter().enumerate() {
            let named = link.kind == LinkKind::Wiki && names.iter().any(|n| wiki_names(link, n));
            if affected.contains(&note) || named {
                candidates.insert((note, i));
            }
        }
    }

    let mut plan = Plan::default();
    let mut order: Vec<(Uuid, usize)> = candidates.into_iter().collect();
    order.sort_by_cached_key(|&(note, i)| (files.path(note), i));
    for (note, i) in order {
        let Some(link) = index.link(note, i) else { continue };
        if matches!(change, Change::Delete { .. }) && affected.contains(&note) || to_itself(link) {
            continue;
        }
        let Some((target, reach)) = locate(files, note, link) else { continue };
        if locate(&after, note, link).map(|(id, _)| id) == Some(target) {
            continue;
        }
        let stuck = || Backlink { note, link: link.clone() };
        if after.get_by_id(target).is_none() || !link.rewritable || !can_edit(files, note) {
            plan.stuck.push(stuck());
            continue;
        }
        match rewrite(&after, note, link, target, reach) {
            Some(to) => {
                plan.mends
                    .push(Mend { note, kind: link.kind, from: link.dest.clone(), to })
            }
            None => plan.stuck.push(stuck()),
        }
    }
    plan
}

/// Whether a link names only a heading of its own note (`#goals`,
/// `[[#Goals]]`), which no change to the tree moves.
fn to_itself(link: &NoteLink) -> bool {
    match link.kind {
        LinkKind::Wiki => wiki_fragment(&link.dest).0.is_empty(),
        _ => split_fragment(&link.dest).0.is_empty(),
    }
}

/// What sharing `id` does to the links inside it, under the scope the share
/// makes: those that open the same file for everyone now and won't then,
/// and, for files the user owns, id links to share them by.
fn share_plan(files: &FileCache, after: &FileCache, id: Uuid) -> Plan {
    let mut inside: Vec<Uuid> = files.descendents(id).iter().map(|f| f.id).collect();
    inside.push(id);
    inside.sort_by_key(|id| files.path(*id));
    let me = &files.root().owner;
    let mut plan = Plan::default();
    for note in inside {
        let Some(text) = files.notes.note(note) else { continue };
        for link in &text.links {
            let Some((target, Reach::Scope)) = locate(files, note, link) else { continue };
            if matches!(locate(after, note, link), Some((_, Reach::Scope))) {
                continue;
            }
            plan.stuck.push(Backlink { note, link: link.clone() });
            let owned = files.get_by_id(target).is_some_and(|f| f.owner == *me);
            if !owned || link.kind == LinkKind::Wiki || !link.rewritable || !can_edit(files, note) {
                continue;
            }
            if !plan.share_too.contains(&target) {
                plan.share_too.push(target);
            }
            let fragment = split_fragment(&link.dest).1;
            let to = match fragment {
                Some(f) => format!("lb://{target}#{f}"),
                None => format!("lb://{target}"),
            };
            plan.mends
                .push(Mend { note, kind: link.kind, from: link.dest.clone(), to });
        }
    }
    plan
}

/// Whether a wiki link's title could name a file called `name`.
fn wiki_names(link: &NoteLink, name: &str) -> bool {
    let title = wiki_fragment(&link.dest).0;
    let last = title.rsplit('/').next().unwrap_or(title).trim();
    title_matches(name, last) || title_matches(name, &link.dest)
}

/// The destination that reaches `target` from `note` in `after`, in the
/// link's own form, with its heading. None when no such destination does.
fn rewrite(
    after: &FileCache, note: Uuid, link: &NoteLink, target: Uuid, reach: Reach,
) -> Option<String> {
    let to = match link.kind {
        LinkKind::Wiki => {
            let heading = (after.wiki_file_part(&link.dest) != link.dest)
                .then(|| wiki_fragment(&link.dest).1)
                .flatten();
            let title = match reach {
                Reach::Scope => after.wikilink_title(target, note),
                Reach::Reader => after.wikilink_title_in(target, after.tree_root(note)),
            }?;
            match heading {
                Some(h) => format!("{title}#{h}"),
                None => title,
            }
        }
        LinkKind::Link | LinkKind::Image => {
            let (path, fragment) = split_fragment(&link.dest);
            let path = if path.starts_with('/') {
                encode_link_path(&after.path(target))
            } else {
                after.link_destination(target, note).unwrap_or_else(|| {
                    let from = after.get_by_id(note).map(|f| f.parent).unwrap_or(note);
                    encode_link_path(&relative_path(&after.path(from), &after.path(target)))
                })
            };
            match fragment {
                Some(f) => format!("{path}#{f}"),
                None => path,
            }
        }
    };
    let mended = NoteLink { dest: to.clone(), ..link.clone() };
    (locate(after, note, &mended).map(|(id, _)| id) == Some(target)).then_some(to)
}

/// Writes `mends` into their notes: each destination written as `from` is
/// rewritten `to`, leaving the rest of the note as it is. A note changed
/// since is rewritten from what it holds now. Returns the notes it couldn't
/// write.
pub fn write_mends(lb: &Lb, mends: &[Mend]) -> Vec<(Uuid, String)> {
    let mut failed = vec![];
    for note in distinct(mends.iter().map(|m| m.note)) {
        let ours: Vec<&Mend> = mends.iter().filter(|m| m.note == note).collect();
        if let Err(e) = write_note(lb, note, &ours) {
            failed.push((note, e.to_string()));
        }
    }
    failed
}

fn write_note(lb: &Lb, note: Uuid, mends: &[&Mend]) -> LbResult<()> {
    let mut written: Vec<Mend> = vec![];
    for m in mends {
        let to = match m.to.strip_prefix("lb://") {
            Some(id) => {
                let (id, fragment) = id.split_once('#').map_or((id, None), |(i, f)| (i, Some(f)));
                let id = Uuid::parse_str(id).unwrap_or_default();
                let url = lb.get_file_link_url(id)?;
                fragment.map_or(url.clone(), |f| format!("{url}#{f}"))
            }
            None => m.to.clone(),
        };
        written.push(Mend { to, ..(*m).clone() });
    }
    let mends: Vec<&Mend> = written.iter().collect();
    // a write between our read and write sends us around again
    loop {
        let (hmac, bytes) = lb.read_document_with_hmac(note, false)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let mended = mend_text(&text, &mends);
        if mended == text {
            return Ok(());
        }
        match lb.safe_write(note, hmac, mended.into_bytes(), None) {
            Err(e) if e.kind == lb_rs::model::errors::LbErrKind::ReReadRequired => continue,
            other => return other.map(|_| ()),
        }
    }
}

/// `text` with every destination a mend names rewritten.
pub fn mend_text(text: &str, mends: &[&Mend]) -> String {
    let mut out = text.to_string();
    let mut dests = written_dests(text);
    dests.sort_by_key(|(_, _, at)| std::cmp::Reverse(at.start));
    for (kind, dest, at) in dests {
        if let Some(m) = mends.iter().find(|m| m.kind == kind && m.from == dest) {
            out.replace_range(at, &m.to);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::read_note;
    use crate::test_utils::files::{at, cache, tree};
    use lb_rs::model::file::ShareMode;

    /// A vault: `(path, text)` notes, plus files with no text.
    fn vault(notes: &[(&str, &str)], others: &[&str]) -> (FileCache, Vec<(Uuid, String)>) {
        let mut paths: Vec<&str> = notes.iter().map(|(p, _)| *p).collect();
        paths.extend(others);
        let files = tree("alice", &paths);
        let mut c = cache(files);
        let texts: Vec<(Uuid, String)> = notes
            .iter()
            .map(|(p, t)| (at(&c, p), t.to_string()))
            .collect();
        for (id, text) in &texts {
            c.notes.insert(*id, read_note(text));
        }
        c.notes.set_ready();
        (c, texts)
    }

    /// Makes `change` with every mend accepted, and checks the invariant
    /// mends exist for: every link reaches what it reached before, or is
    /// listed as stuck. Returns the notes' new texts.
    fn accept_all(
        files: &FileCache, texts: &[(Uuid, String)], change: &Change,
    ) -> (Plan, Vec<(Uuid, String)>) {
        let plan = plan(files, change);
        let mut after = files.after(change);
        let mut mended = vec![];
        for (id, text) in texts {
            let mine: Vec<&Mend> = plan.mends.iter().filter(|m| m.note == *id).collect();
            let text = mend_text(text, &mine);
            if after.get_by_id(*id).is_some() {
                after.notes.insert(*id, read_note(&text));
            }
            mended.push((*id, text));
        }
        for (id, text) in texts {
            if after.get_by_id(*id).is_none() {
                continue;
            }
            let before = read_note(text);
            let now = after.notes.note(*id).unwrap();
            for (b, a) in before.links.iter().zip(&now.links) {
                let Some((target, _)) = locate(files, *id, b) else { continue };
                let still = locate(&after, *id, a).map(|(t, _)| t) == Some(target);
                let stuck = plan
                    .stuck
                    .iter()
                    .any(|s| s.note == *id && s.link.dest == b.dest);
                assert!(
                    still || stuck,
                    "{} in {} lost {}",
                    b.dest,
                    files.path(*id),
                    files.path(target)
                );
            }
        }
        (plan, mended)
    }

    fn text(texts: &[(Uuid, String)], files: &FileCache, path: &str) -> String {
        let id = at(files, path);
        texts.iter().find(|(i, _)| *i == id).unwrap().1.clone()
    }

    #[test]
    fn a_rename_mends_every_form_and_keeps_headings() {
        let (files, texts) = vault(
            &[
                ("/notes/plan.md", "# Goals"),
                (
                    "/notes/log.md",
                    "[p](plan.md#goals) [[plan#Goals|the plan]] [abs](/notes/plan.md) \
                     [[other]] [web](https://plan.md)",
                ),
                ("/notes/other.md", ""),
            ],
            &[],
        );
        let id = at(&files, "/notes/plan.md");
        let change = Change::Rename { id, name: "roadmap 2.md".into() };
        let (plan, texts) = accept_all(&files, &texts, &change);
        assert_eq!(plan.mended_notes(), [at(&files, "/notes/log.md")]);
        assert_eq!(
            text(&texts, &files, "/notes/log.md"),
            "[p](roadmap%202.md#goals) [[roadmap 2#Goals|the plan]] [abs](/notes/roadmap%202.md) \
             [[other]] [web](https://plan.md)"
        );
    }

    /// A note's links to itself by name follow a rename; one to a heading
    /// alone needs nothing. Every occurrence counts, and a reference link,
    /// whose destination is written elsewhere, is listed as breaking.
    #[test]
    fn a_rename_mends_self_links_and_counts_each_link() {
        let (files, texts) = vault(
            &[
                ("/notes/plan.md", "[top](plan.md#goals) [[plan#Goals]] [h](#goals)"),
                ("/notes/log.md", "[x](plan.md) [x](plan.md) [ref][r]\n\n[r]: plan.md\n"),
            ],
            &[],
        );
        let change = Change::Rename { id: at(&files, "/notes/plan.md"), name: "roadmap.md".into() };
        let (plan, texts) = accept_all(&files, &texts, &change);
        assert_eq!(plan.mends.len(), 4);
        assert_eq!(plan.stuck_notes(), [at(&files, "/notes/log.md")]);
        assert_eq!(
            text(&texts, &files, "/notes/plan.md"),
            "[top](roadmap.md#goals) [[roadmap#Goals]] [h](#goals)"
        );
        assert_eq!(
            text(&texts, &files, "/notes/log.md"),
            "[x](roadmap.md) [x](roadmap.md) [ref][r]\n\n[r]: plan.md\n"
        );
    }

    /// A moved note's own relative links follow it, and links to it.
    #[test]
    fn a_move_mends_links_out_of_and_into_the_note() {
        let (files, texts) = vault(
            &[
                ("/a/note.md", "[x](x.md) [y](../b/y.md) [[x]]"),
                ("/a/x.md", "[back](note.md)"),
                ("/b/y.md", ""),
            ],
            &["/c/"],
        );
        let change = Change::Move { ids: vec![at(&files, "/a/note.md")], to: at(&files, "/c/") };
        let (_, texts) = accept_all(&files, &texts, &change);
        assert_eq!(text(&texts, &files, "/a/note.md"), "[x](../a/x.md) [y](../b/y.md) [[x]]");
        assert_eq!(text(&texts, &files, "/a/x.md"), "[back](../c/note.md)");
    }

    /// Renaming a file to a title a wiki link already uses would take the
    /// link; the mend names the file it meant more fully.
    #[test]
    fn a_rename_that_captures_a_wiki_link_qualifies_it() {
        let (files, texts) = vault(
            &[("/notes/log.md", "[[Plan]]"), ("/deep/er/Plan.md", ""), ("/notes/draft.md", "")],
            &[],
        );
        let change = Change::Rename { id: at(&files, "/notes/draft.md"), name: "Plan.md".into() };
        let (_, texts) = accept_all(&files, &texts, &change);
        assert_eq!(text(&texts, &files, "/notes/log.md"), "[[er/Plan]]");
    }

    /// A new note can take a wiki link the same way; one nothing matched
    /// yet is left to find it.
    #[test]
    fn a_create_qualifies_the_links_it_would_take() {
        let (files, texts) =
            vault(&[("/notes/log.md", "[[Plan]] [[Later]]"), ("/deep/er/Plan.md", "")], &[]);
        let change = Change::Create { parent: at(&files, "/notes/"), name: "Plan.md".into() };
        let (plan, texts) = accept_all(&files, &texts, &change);
        assert_eq!(plan.mends.len(), 1);
        assert_eq!(text(&texts, &files, "/notes/log.md"), "[[er/Plan]] [[Later]]");
    }

    /// A folder's rename reaches links to everything under it.
    #[test]
    fn a_folder_rename_mends_links_into_it() {
        let (files, texts) = vault(
            &[("/log.md", "![](imgs/a.png) [b](imgs/sub/b.md)")],
            &["/imgs/a.png", "/imgs/sub/b.md"],
        );
        let change = Change::Rename { id: at(&files, "/imgs/"), name: "media".into() };
        let (_, texts) = accept_all(&files, &texts, &change);
        assert_eq!(text(&texts, &files, "/log.md"), "![](media/a.png) [b](media/sub/b.md)");
    }

    #[test]
    fn a_delete_lists_the_links_it_breaks() {
        let (files, texts) = vault(
            &[("/log.md", "[[plan]] [x](x.md)"), ("/plan.md", "[[log]]"), ("/x.md", "")],
            &[],
        );
        let change = Change::Delete { ids: vec![at(&files, "/plan.md")] };
        let (plan, _) = accept_all(&files, &texts, &change);
        assert!(plan.mends.is_empty());
        assert_eq!(plan.stuck_notes(), [at(&files, "/log.md")]);
    }

    /// Sharing a folder: links out of it stop opening the same file for
    /// everyone, and those to the sharer's own files can become id links.
    #[test]
    fn a_share_lists_links_leaving_it() {
        let (files, texts) = vault(
            &[
                ("/project/plan.md", "[b](../budget.md#q4) [[budget]] [in](notes.md) [[notes]]"),
                ("/project/notes.md", ""),
                ("/budget.md", ""),
            ],
            &[],
        );
        let id = at(&files, "/project/");
        let change = Change::Share { id, with: "bob".into(), mode: ShareMode::Read };
        let plan = plan(&files, &change);
        let stuck: Vec<&str> = plan.stuck.iter().map(|b| b.link.dest.as_str()).collect();
        assert_eq!(stuck, ["../budget.md#q4", "budget"]);
        assert_eq!(plan.share_too, [at(&files, "/budget.md")]);
        let budget = at(&files, "/budget.md");
        let mine: Vec<&Mend> = plan.mends.iter().collect();
        assert_eq!(
            mend_text(&texts[0].1, &mine),
            format!("[b](lb://{budget}#q4) [[budget]] [in](notes.md) [[notes]]")
        );
    }

    /// Until every note has been read, a plan says it may be missing links,
    /// so a drop asks first.
    #[test]
    fn a_plan_before_every_note_is_read_says_so() {
        let c = cache(tree("alice", &["/a.md", "/b.md"]));
        let change = Change::Rename { id: at(&c, "/a.md"), name: "c.md".into() };
        assert!(plan(&c, &change).checking && !plan(&c, &change).is_empty());
        c.notes.set_ready();
        assert!(plan(&c, &change).is_empty());
    }

    /// Notes the reader can't write are listed, not edited.
    #[test]
    fn links_in_notes_the_reader_cant_write_are_stuck() {
        let mut files = tree("alice", &["/shared/", "/shared/theirs.md", "/shared/plan.md"]);
        for f in files.iter_mut().skip(1) {
            f.owner = "bob".into();
        }
        let mut c = cache(files);
        let theirs = at(&c, "/shared/theirs.md");
        c.notes.insert(theirs, read_note("[[plan]]"));
        let change = Change::Rename { id: at(&c, "/shared/plan.md"), name: "q.md".into() };
        let plan = plan(&c, &change);
        assert!(plan.mends.is_empty());
        assert_eq!(plan.stuck_notes(), [theirs]);
    }
}
