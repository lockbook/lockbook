use std::iter;
use std::sync::{Arc, RwLock};

use lb_rs::Uuid;

use crate::file_cache::{FileCache, FilesExt, Reach, link_id, web_link};
#[cfg(test)]
use lb_rs::model::file::File;
use lb_rs::model::file::ShareMode;

pub use crate::file_cache::ResolvedLink;

/// Visual state for a link, used to color the link text and potentially show
/// a hover tooltip explaining the state.
#[derive(Clone, PartialEq, Eq)]
pub enum LinkState {
    Normal,
    Warning { message: String },
    Broken { message: String },
}

pub trait LinkResolver {
    /// Resolve a markdown link URL to either a lockbook file or external URL.
    fn resolve_link(&self, url: &str) -> Option<ResolvedLink>;

    /// Resolve a wikilink target (e.g. `[[Notes]]`) to a file id.
    fn resolve_wikilink(&self, title: &str) -> Option<Uuid>;

    /// State of the given markdown link URL for display and hover tooltips.
    fn link_state(&self, url: &str) -> LinkState;

    /// State of the given wikilink target for display and hover tooltips.
    fn wikilink_state(&self, title: &str) -> LinkState;

    /// Why the file this URL reaches should open only once the reader
    /// confirms it: the link doesn't open the same file for every reader.
    fn confirmation(&self, _url: &str) -> Option<String> {
        None
    }

    /// [`Self::confirmation`] for a wikilink target.
    fn wikilink_confirmation(&self, _title: &str) -> Option<String> {
        None
    }
}

/// No files: only web URLs resolve.
impl LinkResolver for () {
    fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        web_link(url).then(|| ResolvedLink::External(url.to_string()))
    }
    fn resolve_wikilink(&self, _title: &str) -> Option<Uuid> {
        None
    }
    fn link_state(&self, _url: &str) -> LinkState {
        LinkState::Normal
    }
    fn wikilink_state(&self, _title: &str) -> LinkState {
        LinkState::Normal
    }
}

/// Resolver backed by lockbook's file cache, for links in the note
/// `file_id`. A link inside the note's scope means the same file to every
/// reader; see [`FilesExt::locate_link`].
#[derive(Clone)]
pub struct FileCacheLinkResolver {
    files: Arc<RwLock<FileCache>>,
    file_id: Uuid,
}

impl FileCacheLinkResolver {
    pub fn new(files: Arc<RwLock<FileCache>>, file_id: Uuid) -> Self {
        Self { files, file_id }
    }
}

impl LinkResolver for FileCacheLinkResolver {
    fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        self.files.read().unwrap().resolve_link(url, self.file_id)
    }

    fn resolve_wikilink(&self, title: &str) -> Option<Uuid> {
        self.files
            .read()
            .unwrap()
            .resolve_wikilink(title, self.file_id)
    }

    fn link_state(&self, url: &str) -> LinkState {
        let files = self.files.read().unwrap();
        let note = self.file_id;
        match files.locate_link(url, note) {
            Some((_, Reach::Scope)) => LinkState::Normal,
            Some(_) => LinkState::Warning {
                message: tooltip(&*files, note, yellow(&*files, note, Form::of(url))),
            },
            None if files.resolve_link(url, note).is_some() => LinkState::Normal,
            None => {
                let why = if link_id(url).is_some() {
                    Why::new(
                        "This file was deleted or isn't shared with you.",
                        "Ask the owner to share it or remove the link.",
                    )
                } else {
                    not_found(&*files, note, "No file you can see is at this location.")
                };
                LinkState::Broken { message: tooltip(&*files, note, why) }
            }
        }
    }

    fn wikilink_state(&self, title: &str) -> LinkState {
        let files = self.files.read().unwrap();
        let note = self.file_id;
        match files.locate_wikilink(title, note) {
            Some((_, Reach::Scope)) => LinkState::Normal,
            Some(_) => LinkState::Warning {
                message: tooltip(&*files, note, yellow(&*files, note, Form::Path)),
            },
            None => {
                let tied = |scope| files.wikilink_nearest_in(title, note, scope).len() > 1;
                let why = if tied(files.scope_top(note)) || tied(files.tree_root(note)) {
                    let last = title.rsplit('/').next().unwrap_or(title);
                    Why::new(
                        "More than one file has this name.",
                        format!("Add its folder to the link: [[folder/**{last}**]]."),
                    )
                } else {
                    not_found(&*files, note, "No file you can see has this name.")
                };
                LinkState::Broken { message: tooltip(&*files, note, why) }
            }
        }
    }

    fn confirmation(&self, url: &str) -> Option<String> {
        let files = self.files.read().unwrap();
        let note = self.file_id;
        let (_, reach) = files.locate_link(url, note)?;
        (reach != Reach::Scope).then(|| yellow(&*files, note, Form::of(url)).line)
    }

    fn wikilink_confirmation(&self, title: &str) -> Option<String> {
        let files = self.files.read().unwrap();
        let note = self.file_id;
        let (_, reach) = files.locate_wikilink(title, note)?;
        (reach != Reach::Scope).then(|| yellow(&*files, note, Form::Path).line)
    }
}

/// A message line as (text, bold) spans: the parts filled in from the note
/// (a folder, a title) are written `**like this**`.
pub fn rich_spans(line: &str) -> Vec<(&str, bool)> {
    line.split("**")
        .enumerate()
        .filter(|(_, part)| !part.is_empty())
        .map(|(i, part)| (part, i % 2 == 1))
        .collect()
}

/// How a destination is written, which decides what its warning says.
#[derive(Clone, Copy)]
enum Form {
    Id,
    Absolute,
    /// A relative path or a wiki title.
    Path,
}

impl Form {
    fn of(url: &str) -> Self {
        if link_id(url).is_some() {
            Form::Id
        } else if url.starts_with('/') {
            Form::Absolute
        } else {
            Form::Path
        }
    }
}

/// What's wrong with a link and who it affects, and what would fix it.
struct Why {
    line: String,
    fix: String,
}

impl Why {
    fn new(line: impl Into<String>, fix: impl Into<String>) -> Self {
        Self { line: line.into(), fix: fix.into() }
    }
}

/// The tooltip: what's wrong, then the fix for a reader who can edit the note.
fn tooltip(files: &impl FilesExt, note: Uuid, why: Why) -> String {
    if can_edit(files, note) { format!("{}\n{}", why.line, why.fix) } else { why.line }
}

/// A yellow link: who can't open it and why.
fn yellow(files: &impl FilesExt, note: Uuid, form: Form) -> Why {
    if let Form::Absolute = form {
        return Why::new(
            "Each reader opens whatever is at this location in their own files.",
            "Link to the file itself so everyone gets the same one.",
        );
    }
    let (line, fix) = match (shared_folder(files, note), form) {
        (Some(folder), Form::Path) => (
            format!("Readers who only have **{folder}** may not be able to open this."),
            format!("Move the file into **{folder}** and update the link."),
        ),
        (Some(folder), _) => (
            format!("Readers who only have **{folder}** may not be able to open this."),
            format!("Move the file into **{folder}** or share it with everyone there."),
        ),
        (None, Form::Path) => (
            "Readers who only have this note may not be able to open this.".into(),
            "Put this note and the file in one folder and share that instead.".into(),
        ),
        (None, _) => (
            "Readers who only have this note may not be able to open this.".into(),
            "Share the file with everyone who reads this note.".into(),
        ),
    };
    Why::new(line, fix)
}

/// A red link that names nothing the reader can see; it may exist for others.
fn not_found(files: &impl FilesExt, note: Uuid, line: &str) -> Why {
    let me = &files.root().owner;
    let fix: String = match files.get_by_id(note).map(|f| &f.owner) {
        Some(owner) if owner != me => {
            "Check the spelling or ask the note's owner if it's in a folder you don't have.".into()
        }
        _ => "Check the spelling or create the file.".into(),
    };
    Why::new(line, fix)
}

/// The folder shared around the note; None when the note was shared on its own.
fn shared_folder(files: &impl FilesExt, note: Uuid) -> Option<&str> {
    files
        .get_by_id(files.scope_top(note))
        .filter(|f| f.is_folder())
        .map(|f| f.name.as_str())
}

/// Whether the reader can change the note: they own it or hold it with
/// write. A placed share's folders above it are the reader's own, so only
/// shares count there.
fn can_edit(files: &impl FilesExt, note: Uuid) -> bool {
    let me = &files.root().owner;
    let owns = files.get_by_id(note).is_some_and(|f| f.owner == *me);
    owns || iter::once(note)
        .chain(files.ancestors(note))
        .filter_map(|id| files.get_by_id(id))
        .any(|f| {
            f.shares
                .iter()
                .any(|s| s.shared_with == *me && s.mode == ShareMode::Write)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_cache::tests::alice;

    fn message(state: LinkState) -> String {
        match state {
            LinkState::Normal => "normal".into(),
            LinkState::Warning { message } => format!("warning: {message}"),
            LinkState::Broken { message } => format!("broken: {message}"),
        }
    }

    #[test]
    fn states_say_why() {
        let (cache, [note, project, own_budget, ..]) = alice();
        let alice_files: Vec<File> = cache.iter_files().cloned().collect();
        let links = FileCacheLinkResolver::new(Arc::new(RwLock::new(cache)), note);
        let own = format!("lb://{own_budget}");
        let unknown = "https://app.lockbook.net/open/00000000-0000-0000-0000-00000000002a";
        let line2 = |line: &str, fix: &str| format!("{line}\n{fix}");
        for (url, expected, asks) in [
            (
                own.as_str(),
                format!(
                    "warning: {}",
                    line2(
                        "Readers who only have **project** may not be able to open this.",
                        "Move the file into **project** or share it with everyone there."
                    )
                ),
                true,
            ),
            ("../Budget.md", "normal".into(), false),
            (
                "../../personal/Budget.md",
                format!(
                    "warning: {}",
                    line2(
                        "Readers who only have **project** may not be able to open this.",
                        "Move the file into **project** and update the link."
                    )
                ),
                true,
            ),
            (
                "/finance/budget.md",
                format!(
                    "warning: {}",
                    line2(
                        "Each reader opens whatever is at this location in their own files.",
                        "Link to the file itself so everyone gets the same one."
                    )
                ),
                true,
            ),
            (
                unknown,
                format!(
                    "broken: {}",
                    line2(
                        "This file was deleted or isn't shared with you.",
                        "Ask the owner to share it or remove the link."
                    )
                ),
                false,
            ),
            (
                "missing.md",
                format!(
                    "broken: {}",
                    line2(
                        "No file you can see is at this location.",
                        "Check the spelling or create the file."
                    )
                ),
                false,
            ),
        ] {
            assert_eq!(message(links.link_state(url)), expected, "{url}");
            assert_eq!(links.confirmation(url).is_some(), asks, "{url}");
        }
        assert_eq!(
            links.confirmation(&own).as_deref(),
            Some("Readers who only have **project** may not be able to open this.")
        );
        assert_eq!(message(links.wikilink_state("Budget")), "normal");
        assert_eq!(links.wikilink_confirmation("Budget"), None);
        assert!(
            message(links.wikilink_state("../../personal/Budget"))
                .starts_with("warning: Readers who only have **project**")
        );
        assert!(
            links
                .wikilink_confirmation("../../personal/Budget")
                .is_some()
        );
        assert_eq!(
            message(links.wikilink_state("Nothing")),
            format!(
                "broken: {}",
                line2(
                    "No file you can see has this name.",
                    "Check the spelling or create the file."
                )
            )
        );

        // Bob holds the project pending, and has a private file of his own
        let bob = Uuid::from_u128(100);
        let private = Uuid::from_u128(101);
        let mut bob_root = alice_files[0].clone();
        bob_root.id = bob;
        bob_root.parent = bob;
        bob_root.owner = "bob".into();
        let mut secret = bob_root.clone();
        secret.id = private;
        secret.name = "secret.md".into();
        secret.file_type = lb_rs::model::file_metadata::FileType::Document;
        // read-only, so no fix lines
        let shared: Vec<File> = alice_files
            .iter()
            .filter(|f| f.id == project || alice_files.ancestors(f.id).contains(&project))
            .cloned()
            .map(|mut f| {
                f.shares.iter_mut().for_each(|s| s.mode = ShareMode::Read);
                f
            })
            .collect();
        // and the same share placed in his own root, where every folder above
        // the share is his but the note is still read-only
        let mut placed = shared.clone();
        placed
            .iter_mut()
            .filter(|f| f.id == project)
            .for_each(|f| f.parent = bob);
        let pending = FileCache::from_owned_and_shared(
            bob_root.clone(),
            [bob_root.clone(), secret.clone()],
            shared,
        );
        let placed = FileCache::from_owned_and_shared(bob_root.clone(), [bob_root, secret], placed);
        for view in [pending, placed] {
            let links = FileCacheLinkResolver::new(Arc::new(RwLock::new(view)), note);
            assert_eq!(
                message(links.link_state(&own)),
                "broken: This file was deleted or isn't shared with you."
            );
            let secret = format!("lb://{private}");
            let reason = "Readers who only have **project** may not be able to open this.";
            assert_eq!(message(links.link_state(&secret)), format!("warning: {reason}"));
            assert_eq!(
                message(links.link_state("missing.md")),
                "broken: No file you can see is at this location."
            );
            assert_eq!(links.confirmation(&secret).as_deref(), Some(reason));
        }
    }

    #[test]
    fn filled_in_parts_are_bold() {
        assert_eq!(
            rich_spans("Readers who only have **design** may not be able to open this."),
            [
                ("Readers who only have ", false),
                ("design", true),
                (" may not be able to open this.", false)
            ]
        );
        assert_eq!(
            rich_spans("No file you can see is at this location."),
            [("No file you can see is at this location.", false)]
        );
    }

    #[test]
    fn a_tie_says_so() {
        use lb_rs::model::file_metadata::FileType;
        let (cache, [note, project, ..]) = alice();
        let mut files: Vec<File> = cache.iter_files().cloned().collect();
        let template = files.iter().find(|f| f.id == note).unwrap().clone();
        for (id, parent, name, file_type) in [
            (200, project, "a", FileType::Folder),
            (201, project, "b", FileType::Folder),
            (202, Uuid::from_u128(200), "Spec.md", FileType::Document),
            (203, Uuid::from_u128(201), "Spec.md", FileType::Document),
        ] {
            let mut f = template.clone();
            f.id = Uuid::from_u128(id);
            f.parent = parent;
            f.name = name.into();
            f.file_type = file_type;
            files.push(f);
        }
        let mut cache_note = files.iter().find(|f| f.id == note).unwrap().clone();
        cache_note.parent = project;
        files.retain(|f| f.id != note);
        files.push(cache_note);
        let root = files.iter().find(|f| f.is_root()).unwrap().clone();
        let view = FileCache::from_owned_and_shared(root, files, []);
        let links = FileCacheLinkResolver::new(Arc::new(RwLock::new(view)), note);
        assert_eq!(
            message(links.wikilink_state("Spec")),
            "broken: More than one file has this name.\nAdd its folder to the link: [[folder/**Spec**]]."
        );
        assert_eq!(message(links.wikilink_state("a/Spec")), "normal");
    }
}
