//! Keeps links reaching their files as files are renamed, moved, shared,
//! and deleted: which links a change took from their files, and the edits
//! that bring them back.

use std::cmp::Reverse;

use lb_rs::Uuid;

use super::index::reach;
use super::{LinkIndex, LinkKind, Spans, extract};
use crate::file_cache::{FilesExt, ResolvedLink, encode_link_path, escape_wiki_title};
use crate::tab::image_viewer::is_supported_image_fmt;

/// A link that no longer reaches the file it is meant to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stray {
    pub note: Uuid,
    pub kind: LinkKind,
    /// The destination as written.
    pub dest: String,
    pub meant: Uuid,
    /// What to write so the link reaches the file again. `None` when
    /// nothing written would (an embed can't leave its note's scope, nor an
    /// id link its note's owner's reach), or when the destination is written
    /// where it can't be found.
    pub mend: Option<Mend>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mend {
    /// This destination, or wikilink title, in place of the one written.
    Dest(String),
    /// A link with this destination in place of a wikilink, whose title
    /// can't leave the note's scope.
    Link(String),
}

/// A note whose stray links can't be rewritten to reach their files.
#[derive(Debug, PartialEq, Eq)]
pub struct Unmendable;

/// Every stray link, in a stable order.
pub fn strays<F: FilesExt + ?Sized>(files: &F, index: &LinkIndex) -> Vec<Stray> {
    let mut strays: Vec<(String, Stray)> = index
        .strays()
        .filter_map(|(note, link)| {
            let (kind, dest, meant) = (link.link.kind, link.link.dest.clone(), link.meant?);
            let fragment = dest.get(link.named..).unwrap_or_default();
            let located = link.link.spans.as_ref();
            let mend = located.and_then(|_| mend_for(files, note, kind, fragment, meant));
            Some((files.path(note), Stray { note, kind, dest, meant, mend }))
        })
        .collect();
    // one for all that are written alike in a note, mended if any can be
    let key =
        |(path, s): &(String, Stray)| (path.clone(), s.dest.clone(), s.kind, s.mend.is_none());
    strays.sort_by_key(key);
    strays.dedup_by(|a, b| (a.1.note, a.1.kind, &a.1.dest) == (b.1.note, b.1.kind, &b.1.dest));
    strays.into_iter().map(|(_, stray)| stray).collect()
}

/// What to write in `note` so a link of `kind` reaches `meant`, with the
/// `fragment` its destination ends in kept.
fn mend_for<F: FilesExt + ?Sized>(
    files: &F, note: Uuid, kind: LinkKind, fragment: &str, meant: Uuid,
) -> Option<Mend> {
    let reaches = |dest: &str| matches!(files.resolve_link(dest, note), Some(ResolvedLink::File(id)) if id == meant);
    match kind {
        LinkKind::Wiki => match files.wikilink_title(meant, note) {
            Some(title) => Some(Mend::Dest(format!("{title}{}", escape_wiki_title(fragment)))),
            None => {
                // a heading's text, where a destination ends at a space
                let fragment = match fragment.strip_prefix('#') {
                    Some(heading) => format!("#{}", encode_link_path(heading)),
                    None => String::new(),
                };
                let dest = format!("lb://{meant}");
                reaches(&dest).then(|| Mend::Link(format!("{dest}{fragment}")))
            }
        },
        LinkKind::Link | LinkKind::Image => {
            let dest = files.link_destination(meant, note);
            let embeds = kind == LinkKind::Link || !dest.starts_with("lb://");
            (embeds && reaches(&dest)).then(|| Mend::Dest(format!("{dest}{fragment}")))
        }
    }
}

/// `text` with each link written as one of `strays` mended, or `None` when
/// it has none to mend. Fails when the text that results wouldn't have each
/// reach its file.
pub fn mend<F: FilesExt + ?Sized>(
    files: &F, text: &str, strays: &[Stray],
) -> Result<Option<String>, Unmendable> {
    let links = extract(text);
    let mut edits = vec![];
    for (i, link) in links.iter().enumerate() {
        let Some(Spans { link: whole, dest }) = link.spans.clone() else { continue };
        let stray = strays
            .iter()
            .find(|s| s.kind == link.kind && s.dest == link.dest);
        match stray.and_then(|s| Some((s, s.mend.as_ref()?))) {
            Some((stray, Mend::Dest(new))) => edits.push((dest, new.clone(), i, stray)),
            Some((stray, Mend::Link(new))) => {
                // `[[title|label]]`; a wikilink with no label shows its title
                let label = text[dest.end..whole.end - 2].trim();
                let label = label.strip_prefix('|').map_or(&text[dest], str::trim);
                edits.push((whole, format!("[{label}]({new})"), i, stray));
            }
            None => {}
        }
    }
    if edits.is_empty() {
        return Ok(None);
    }

    // from the end, each leaving what is before it where it was
    edits.sort_by_key(|(at, ..)| Reverse(at.start));
    let mut mended = text.to_string();
    let mut reached = text.len();
    for (at, new, ..) in &edits {
        if at.end > reached {
            return Err(Unmendable);
        }
        mended.replace_range(at.clone(), new);
        reached = at.start;
    }

    let read = extract(&mended);
    let holds = |(i, after): (usize, &super::Link)| match edits.iter().find(|e| e.2 == i) {
        Some((.., stray)) => reach(files, stray.note, after).map(|r| r.0) == Some(stray.meant),
        None => (after.kind, &after.dest) == (links[i].kind, &links[i].dest),
    };
    let holds = read.len() == links.len() && read.iter().enumerate().all(holds);
    if holds { Ok(Some(mended)) } else { Err(Unmendable) }
}

/// Whether `file` is an image the editor saved on paste or drop.
pub fn pasted_image<F: FilesExt + ?Sized>(files: &F, file: Uuid) -> bool {
    let Some(file) = files.get_by_id(file).filter(|f| f.is_document()) else { return false };
    let image = file
        .name
        .rsplit_once('.')
        .is_some_and(|(_, ext)| is_supported_image_fmt(ext));
    let folder = files.get_by_id(file.parent);
    image && folder.is_some_and(|folder| folder.name == "imports")
}

/// Whether `file` is a pasted image that no link reaches, nor is meant to.
pub fn orphaned<F: FilesExt + ?Sized>(files: &F, index: &LinkIndex, file: Uuid) -> bool {
    let meant = index.strays().any(|(_, link)| link.meant == Some(file));
    pasted_image(files, file) && index.linkers(file).is_empty() && !meant
}

#[cfg(test)]
mod tests {
    use lb_rs::model::file::File;

    use super::*;
    use crate::test_utils::files::{at, cache, tree};

    /// `/budget.md` and a `/project/` shared with bob.
    fn project() -> Vec<File> {
        tree(
            "alice",
            &[
                "/budget.md",
                "/project/ @bob",
                "/project/plan.md",
                "/project/notes.md",
                "/project/C# notes.md",
                "/project/imports/chart.png",
                "/project/design/sketch.md",
            ],
        )
    }

    fn rename(files: &mut [File], path: &str, name: &str) {
        let id = at(files, path);
        files.iter_mut().find(|f| f.id == id).unwrap().name = name.into();
    }

    fn relocate(files: &mut [File], path: &str, folder: &str) {
        let (id, folder) = (at(files, path), at(files, folder));
        files.iter_mut().find(|f| f.id == id).unwrap().parent = folder;
    }

    /// `/budget.md` comes to be shared on its own with carol.
    fn share_budget(files: &mut Vec<File>) {
        let extra = tree("alice", &["/budget.md @carol"]).remove(1).shares;
        let id = at(files, "/budget.md");
        files.iter_mut().find(|f| f.id == id).unwrap().shares = extra;
    }

    /// The text of the note at `path` after `change`, with every stray link
    /// the change left in it mended.
    fn mended(text: &str, path: &str, change: impl Fn(&mut Vec<File>)) -> String {
        let mut files = project();
        let note = at(&files, path);
        let mut index = LinkIndex::default();
        index.set(&cache(files.clone()), note, 0, extract(text));
        assert_eq!(index.strays().count(), 0, "{text}");
        change(&mut files);
        let files = cache(files);
        index.refresh(&files);
        let strays = strays(&files, &index);
        let mended = mend(&files, text, &strays).unwrap();
        let mended = mended.unwrap_or_else(|| text.to_string());

        // a mended link reaches what it did, and the rest are as they were
        let before: Vec<_> = index.outbound(note).to_vec();
        index.set(&files, note, 1, extract(&mended));
        for (was, is) in before.iter().zip(index.outbound(note)) {
            let stray = strays.iter().find(|s| s.dest == was.link.dest);
            match stray.is_some_and(|s| s.mend.is_some()) {
                true => assert_eq!(is.target, was.meant, "{mended}"),
                false => assert_eq!(is.link.dest, was.link.dest, "{mended}"),
            }
        }
        // and none of the mended is a stray again
        let unmended = strays.iter().filter(|s| s.mend.is_none()).count();
        assert_eq!(index.strays().count(), unmended, "{mended}");
        mended
    }

    #[test]
    fn links_follow_a_renamed_or_moved_file() {
        let text = "[the plan](plan.md#goals \"t\"), [[plan]], [[plan|it]], ![](imports/chart.png)";
        assert_eq!(
            mended(text, "/project/notes.md", |f| rename(f, "/project/plan.md", "road map.md")),
            "[the plan](road%20map.md#goals \"t\"), [[road map]], [[road map|it]], ![](imports/chart.png)"
        );
        assert_eq!(
            mended(text, "/project/notes.md", |f| {
                relocate(f, "/project/plan.md", "/project/design");
                relocate(f, "/project/imports/chart.png", "/project/design");
            }),
            "[the plan](design/plan.md#goals \"t\"), [[plan]], [[plan|it]], ![](design/chart.png)"
        );
    }

    #[test]
    fn a_moved_note_keeps_its_own_links() {
        let text = "[plan](plan.md) ![](imports/chart.png) [[sketch]] <https://lockbook.net>";
        let into_design = |f: &mut Vec<File>| relocate(f, "/project/notes.md", "/project/design");
        assert_eq!(
            mended(text, "/project/notes.md", into_design),
            "[plan](../plan.md) ![](../imports/chart.png) [[sketch]] <https://lockbook.net>"
        );
        // an image that is itself a link: both destinations, either way
        assert_eq!(
            mended("[![chart](imports/chart.png)](plan.md)", "/project/notes.md", into_design),
            "[![chart](../imports/chart.png)](../plan.md)"
        );
        assert_eq!(
            mended(
                "[![chart](../imports/chart.png)](../plan.md)",
                "/project/design/sketch.md",
                |f| { relocate(f, "/project/design/sketch.md", "/project") }
            ),
            "[![chart](imports/chart.png)](plan.md)"
        );
    }

    /// One file takes the name another had: each link follows its own file.
    #[test]
    fn links_follow_files_that_trade_names() {
        let text = "[a](../plan.md) [b](../notes.md) [[plan]] [[notes]]";
        let trade = |f: &mut Vec<File>| {
            rename(f, "/project/plan.md", "plan-old.md");
            rename(f, "/project/notes.md", "plan.md");
        };
        assert_eq!(
            mended(text, "/project/design/sketch.md", trade),
            "[a](../plan-old.md) [b](../plan.md) [[plan-old]] [[plan]]"
        );
    }

    #[test]
    fn a_fragment_stays_on_its_link() {
        // a `#` may be the name's own
        let text =
            "[[C# notes]] [[C# notes#Generics \\[draft\\]|generics]] [c](C%23%20notes.md#generics)";
        assert_eq!(
            mended(text, "/project/notes.md", |f| rename(f, "/project/C# notes.md", "C# guide.md")),
            "[[C# guide]] [[C# guide#Generics \\[draft\\]|generics]] [c](C%23%20guide.md#generics)"
        );
    }

    #[test]
    fn a_share_sends_links_out_of_it_by_id() {
        let text = "[plan](project/plan.md#goals) [[plan]] [[plan#Open questions (1)|the open ones]] ![](project/imports/chart.png)";
        let files = project();
        let (plan, chart) =
            (at(&files, "/project/plan.md"), at(&files, "/project/imports/chart.png"));
        assert_eq!(
            mended(text, "/budget.md", share_budget),
            format!(
                "[plan](lb://{plan}#goals) [plan](lb://{plan}) [the open ones](lb://{plan}#Open%20questions%20%281%29) ![](project/imports/chart.png)"
            )
        );

        // the embed can't follow; it is a stray with nothing to write
        let mut shared = project();
        share_budget(&mut shared);
        let mut index = LinkIndex::default();
        index.set(&files, at(&files, "/budget.md"), 0, extract(text));
        index.refresh(&shared);
        let embed = strays(&shared, &index)
            .into_iter()
            .find(|s| s.kind == LinkKind::Image)
            .unwrap();
        assert_eq!((embed.meant, embed.mend), (chart, None));
    }

    #[test]
    fn a_captured_wikilink_is_qualified() {
        let text = "[[plan]] and [[sketch]]";
        let nearer = |files: &mut Vec<File>| {
            let mut f = tree("alice", &["/plan.md"]).remove(1);
            (f.id, f.parent) = (Uuid::from_u128(99), at(files, "/project/design"));
            files.push(f);
        };
        assert_eq!(mended(text, "/project/design/sketch.md", nearer), "[[/plan]] and [[sketch]]");
    }

    /// A destination written apart from its link is left for a hand to
    /// mend; one a rewrite would misread is not written.
    #[test]
    fn what_cant_be_rewritten_isnt() {
        let mut files = project();
        let notes = at(&files, "/project/notes.md");
        let text = "[the plan][p] and [plan](plan.md)\n\n[p]: plan.md\n";
        let mut index = LinkIndex::default();
        index.set(&files, notes, 0, extract(text));
        rename(&mut files, "/project/plan.md", "roadmap.md");
        index.refresh(&files);
        let strays = strays(&files, &index);
        let mends: Vec<_> = strays.iter().map(|s| s.mend.clone()).collect();
        assert_eq!(mends, [Some(Mend::Dest("roadmap.md".into()))]);
        let mended = "[the plan][p] and [plan](roadmap.md)\n\n[p]: plan.md\n";
        assert_eq!(mend(&files, text, &strays), Ok(Some(mended.into())));
        // what is left is the reference, which nothing can be written for
        index.set(&files, notes, 1, extract(mended));
        let left: Vec<_> = super::strays(&files, &index);
        assert_eq!(left.iter().map(|s| &s.mend).collect::<Vec<_>>(), [&None]);
        assert_eq!(mend(&files, mended, &left), Ok(None));

        // a mend that would not reach the file
        let wrong = Stray { mend: Some(Mend::Dest("nowhere.md".into())), ..strays[0].clone() };
        assert_eq!(mend(&files, text, &[wrong]), Err(Unmendable));
        // nothing here to mend
        assert_eq!(mend(&files, "no links", &strays), Ok(None));
    }

    #[test]
    fn pasted_images_nothing_links_to_are_orphans() {
        let files = project();
        let (notes, chart) =
            (at(&files, "/project/notes.md"), at(&files, "/project/imports/chart.png"));
        let mut index = LinkIndex::default();
        index.set(&files, notes, 0, extract("![](imports/chart.png)"));
        assert!(!orphaned(&files, &index, chart));

        assert_eq!(index.set(&files, notes, 1, extract("no chart")), [chart]);
        assert!(orphaned(&files, &index, chart));
        // an embed a move broke still means it
        let mut moved = files.clone();
        relocate(&mut moved, "/project/plan.md", "/project/design");
        index.set(&files, at(&files, "/project/plan.md"), 0, extract("![](imports/chart.png)"));
        index.refresh(&moved);
        assert!(index.linkers(chart).is_empty() && !orphaned(&moved, &index, chart));
        // only pasted images
        assert!(!orphaned(&files, &index, at(&files, "/project/plan.md")));
        assert!(!orphaned(&files, &index, at(&files, "/budget.md")));
    }
}
