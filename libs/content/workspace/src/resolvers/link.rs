use std::sync::{Arc, RwLock};

use lb_rs::Uuid;

use crate::file_cache::{FileCache, FilesExt, strip_ext, title_matches};
use crate::show::DocType;

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

    /// Document the dests in this resolver are relative to. Needed to create
    /// a missing dest as a sibling (or relative path) of the source file.
    fn source_file(&self) -> Option<Uuid> {
        None
    }
}

impl LinkResolver for () {
    fn resolve_link(&self, _url: &str) -> Option<ResolvedLink> {
        None
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

const CROSS_TREE_MSG: &str =
    "This link points to a file shared differently and may not be visible to all collaborators.";

/// Resolver backed by lockbook's file cache. Resolves links relative to the
/// parent folder of a given file. Cross-tree UUID links from a pending share
/// tree are flagged with a yellow warning; the crypto layer enforces access.
#[derive(Clone)]
pub struct FileCacheLinkResolver {
    files: Arc<RwLock<FileCache>>,
    file_id: Uuid,
}

impl FileCacheLinkResolver {
    pub fn new(files: Arc<RwLock<FileCache>>, file_id: Uuid) -> Self {
        Self { files, file_id }
    }

    fn broken_state(&self, dest: &str, is_wikilink: bool) -> LinkState {
        let guard = self.files.read().unwrap();
        let message = if create_spec_for_dest(&*guard, self.file_id, dest, is_wikilink).is_some() {
            "Click to create"
        } else {
            "Destination not found"
        };
        LinkState::Broken { message: message.into() }
    }
}

/// Folders to create, then the document name, under `parent`. The last
/// component is always the file; earlier ones are missing folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateSpec {
    pub parent: Uuid,
    pub components: Vec<String>,
    pub fragment: Option<String>,
}

/// Where to materialize a broken dest, if it is an internal path we can
/// create. Wiki dests without an extension become `.md`. Raster image dests,
/// `http`/`mailto`/`lb://`, same-file fragments, and dests that already match
/// something (including an ambiguous wiki) return None.
pub fn create_spec_for_dest(
    files: &impl FilesExt, from_id: Uuid, dest: &str, is_wikilink: bool,
) -> Option<CreateSpec> {
    let (path, frag) = crate::file_cache::split_internal_fragment(dest);
    if path.is_empty()
        || path.starts_with("http://")
        || path.starts_with("https://")
        || path.starts_with("mailto:")
        || path.starts_with("lb://")
    {
        return None;
    }
    let decoded = urlencoding::decode(path)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| path.to_string());
    if decoded.ends_with('/') {
        return None;
    }

    if is_wikilink && wiki_has_any_match(files, from_id, &decoded) {
        return None;
    }

    let mut components = dest_components(&decoded)?;
    if is_wikilink {
        if let Some(last) = components.last_mut() {
            if strip_ext(last) == last.as_str() {
                last.push_str(".md");
            }
        }
    }
    let last = components.last()?;
    if matches!(DocType::from_name(last), DocType::Image | DocType::ImageUnsupported) {
        return None;
    }

    let start =
        if decoded.starts_with('/') { files.root().id } else { files.get_by_id(from_id)?.parent };

    let mut current = start;
    let mut i = 0;
    while i < components.len() {
        let name = &components[i];
        if name == ".." {
            let f = files.get_by_id(current)?;
            if f.is_root() || files.get_by_id(f.parent).is_none() {
                return None;
            }
            current = f.parent;
            i += 1;
            continue;
        }
        let last = i + 1 == components.len();
        let existing = files
            .children(current)
            .into_iter()
            .find(|f| f.name == *name);
        if last {
            if existing.is_some() {
                return None;
            }
            return Some(CreateSpec {
                parent: current,
                components: components[i..].to_vec(),
                fragment: frag.filter(|s| !s.is_empty()).map(|s| s.to_string()),
            });
        }
        match existing {
            Some(f) if f.is_folder() => {
                current = f.id;
                i += 1;
            }
            Some(_) => return None,
            None => {
                if components[i..].iter().any(|c| c == "..") {
                    return None;
                }
                return Some(CreateSpec {
                    parent: current,
                    components: components[i..].to_vec(),
                    fragment: frag.filter(|s| !s.is_empty()).map(|s| s.to_string()),
                });
            }
        }
    }
    None
}

/// Collapse `.` and internal `..`; keep leading `..` so the tree walk can
/// ascend from the source file's parent.
fn dest_components(path: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|s| s != "..") {
                    out.pop();
                } else {
                    out.push("..".into());
                }
            }
            name => out.push(name.to_string()),
        }
    }
    if out.is_empty() || out.iter().all(|s| s == "..") { None } else { Some(out) }
}

fn wiki_has_any_match(files: &impl FilesExt, from_id: Uuid, title: &str) -> bool {
    let Some(from) = files.get_by_id(from_id) else {
        return false;
    };
    let parent = from.parent;
    if let Some((dir, last)) = title.rsplit_once('/') {
        let Some(dir_id) = walk_rel(files, parent, dir) else {
            return false;
        };
        files
            .children(dir_id)
            .into_iter()
            .any(|f| f.is_document() && title_matches(&f.name, last))
    } else {
        files.iter_files().any(|f| {
            f.is_document() && files.same_tree(parent, f.id) && title_matches(&f.name, title)
        })
    }
}

fn walk_rel(files: &impl FilesExt, mut current: Uuid, rel: &str) -> Option<Uuid> {
    for component in rel.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                let f = files.get_by_id(current)?;
                if f.is_root() || files.get_by_id(f.parent).is_none() {
                    return None;
                }
                current = f.parent;
            }
            name => {
                current = files
                    .children(current)
                    .into_iter()
                    .find(|f| f.name == name)?
                    .id;
            }
        }
    }
    Some(current)
}

impl LinkResolver for FileCacheLinkResolver {
    fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        let (path, frag) = crate::file_cache::split_internal_fragment(url);
        if path.is_empty() {
            return frag.is_some().then_some(ResolvedLink::File(self.file_id));
        }
        let guard = self.files.read().unwrap();
        let from_id = guard.get_by_id(self.file_id)?.parent;
        guard.resolve_link(path, from_id)
    }

    fn resolve_wikilink(&self, title: &str) -> Option<Uuid> {
        let (path, frag) = crate::file_cache::split_internal_fragment(title);
        if path.is_empty() {
            return frag.is_some().then_some(self.file_id);
        }
        let guard = self.files.read().unwrap();
        let from_id = guard.get_by_id(self.file_id)?.parent;
        guard.resolve_wikilink(path, from_id)
    }

    fn link_state(&self, url: &str) -> LinkState {
        match self.resolve_link(url) {
            None => self.broken_state(url, false),
            Some(ResolvedLink::External(_)) => LinkState::Normal,
            Some(ResolvedLink::File(target_id)) => {
                let guard = self.files.read().unwrap();
                let Some(from_id) = guard.get_by_id(self.file_id).map(|f| f.parent) else {
                    return LinkState::Broken { message: "Destination not found".into() };
                };
                let from_own = guard.tree_root(from_id) == guard.root().id;
                if from_own || guard.same_tree(from_id, target_id) {
                    LinkState::Normal
                } else {
                    LinkState::Warning { message: CROSS_TREE_MSG.into() }
                }
            }
        }
    }

    fn wikilink_state(&self, title: &str) -> LinkState {
        match self.resolve_wikilink(title) {
            None => self.broken_state(title, true),
            Some(_) => LinkState::Normal, // wikilinks are always within-tree
        }
    }

    fn source_file(&self) -> Option<Uuid> {
        Some(self.file_id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use lb_rs::Uuid;
    use lb_rs::model::file::File;
    use lb_rs::model::file_metadata::FileType;

    use super::{FileCacheLinkResolver, LinkResolver};
    use crate::file_cache::{FileCache, ResolvedLink, split_internal_fragment};

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

    #[test]
    fn fragment_resolves_to_file() {
        let root_id = Uuid::new_v4();
        let doc_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            file(root_id, root_id, "root", FileType::Folder),
            [
                file(doc_id, root_id, "note.md", FileType::Document),
                file(other_id, root_id, "other.md", FileType::Document),
            ],
            [],
        );
        let resolver = FileCacheLinkResolver::new(Arc::new(RwLock::new(cache)), doc_id);

        assert!(
            matches!(resolver.resolve_link("#heading"), Some(ResolvedLink::File(id)) if id == doc_id)
        );
        assert!(
            matches!(resolver.resolve_link("note.md#heading"), Some(ResolvedLink::File(id)) if id == doc_id)
        );
        assert!(
            matches!(resolver.resolve_link("other.md#x"), Some(ResolvedLink::File(id)) if id == other_id)
        );
        assert!(matches!(
            resolver.resolve_link(&format!("lb://{doc_id}#heading")),
            Some(ResolvedLink::File(id)) if id == doc_id
        ));
        assert!(matches!(
            resolver.resolve_link("https://example.com#h"),
            Some(ResolvedLink::External(u)) if u == "https://example.com#h"
        ));
        assert_eq!(resolver.resolve_wikilink("#heading"), Some(doc_id));
        assert_eq!(resolver.resolve_wikilink("note#heading"), Some(doc_id));
        assert_eq!(resolver.resolve_wikilink("other#x"), Some(other_id));
        assert!(matches!(resolver.link_state("#heading"), super::LinkState::Normal));
        assert!(split_internal_fragment("#heading").0.is_empty());
    }

    fn spec(files: &FileCache, from: Uuid, dest: &str, wiki: bool) -> Option<super::CreateSpec> {
        super::create_spec_for_dest(files, from, dest, wiki)
    }

    #[test]
    fn create_spec_wiki_sibling_adds_md() {
        let root = Uuid::new_v4();
        let doc = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            file(root, root, "root", FileType::Folder),
            [file(doc, root, "a.md", FileType::Document)],
            [],
        );
        let s = spec(&cache, doc, "note", true).unwrap();
        assert_eq!(s.parent, root);
        assert_eq!(s.components, vec!["note.md"]);
        assert!(s.fragment.is_none());
        assert!(matches!(
            FileCacheLinkResolver::new(Arc::new(RwLock::new(cache)), doc).wikilink_state("note"),
            super::LinkState::Broken { message } if message == "Click to create"
        ));
    }

    #[test]
    fn create_spec_wiki_keeps_explicit_ext_and_fragment() {
        let root = Uuid::new_v4();
        let doc = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            file(root, root, "root", FileType::Folder),
            [file(doc, root, "a.md", FileType::Document)],
            [],
        );
        let s = spec(&cache, doc, "sketch.svg#intro", true).unwrap();
        assert_eq!(s.components, vec!["sketch.svg"]);
        assert_eq!(s.fragment.as_deref(), Some("intro"));
        let s = spec(&cache, doc, "note.md", true).unwrap();
        assert_eq!(s.components, vec!["note.md"]);
    }

    #[test]
    fn create_spec_wiki_nested_and_dotdot() {
        let root = Uuid::new_v4();
        let folder = Uuid::new_v4();
        let doc = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            file(root, root, "root", FileType::Folder),
            [
                file(folder, root, "notes", FileType::Folder),
                file(doc, folder, "a.md", FileType::Document),
            ],
            [],
        );
        let nested = spec(&cache, doc, "sub/idea", true).unwrap();
        assert_eq!(nested.parent, folder);
        assert_eq!(nested.components, vec!["sub", "idea.md"]);

        let up = spec(&cache, doc, "../root-note", true).unwrap();
        assert_eq!(up.parent, root);
        assert_eq!(up.components, vec!["root-note.md"]);
    }

    #[test]
    fn create_spec_markdown_relative_and_absolute() {
        let root = Uuid::new_v4();
        let folder = Uuid::new_v4();
        let doc = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            file(root, root, "root", FileType::Folder),
            [
                file(folder, root, "notes", FileType::Folder),
                file(doc, folder, "a.md", FileType::Document),
            ],
            [],
        );
        let rel = spec(&cache, doc, "b.md", false).unwrap();
        assert_eq!(rel.parent, folder);
        assert_eq!(rel.components, vec!["b.md"]);

        let abs = spec(&cache, doc, "/other/c.md", false).unwrap();
        assert_eq!(abs.parent, root);
        assert_eq!(abs.components, vec!["other", "c.md"]);

        // markdown dests keep the name as written — no implied .md
        let bare = spec(&cache, doc, "bare", false).unwrap();
        assert_eq!(bare.components, vec!["bare"]);
    }

    #[test]
    fn create_spec_skips_external_raster_ambiguous_and_existing() {
        let root = Uuid::new_v4();
        let doc = Uuid::new_v4();
        let other = Uuid::new_v4();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let fa = Uuid::new_v4();
        let fb = Uuid::new_v4();
        let cache = FileCache::from_owned_and_shared(
            file(root, root, "root", FileType::Folder),
            [
                file(doc, root, "a.md", FileType::Document),
                file(other, root, "other.md", FileType::Document),
                file(fa, root, "one", FileType::Folder),
                file(fb, root, "two", FileType::Folder),
                file(a, fa, "Spec.md", FileType::Document),
                file(b, fb, "Spec.md", FileType::Document),
            ],
            [],
        );
        assert!(spec(&cache, doc, "https://example.com", false).is_none());
        assert!(spec(&cache, doc, "mailto:x@y.z", false).is_none());
        assert!(spec(&cache, doc, &format!("lb://{doc}"), false).is_none());
        assert!(spec(&cache, doc, "#heading", true).is_none());
        assert!(spec(&cache, doc, "pic.png", false).is_none());
        assert!(spec(&cache, doc, "folder/", false).is_none());
        assert!(spec(&cache, doc, "other", true).is_none()); // already exists
        assert!(spec(&cache, doc, "Spec", true).is_none()); // ambiguous stem
        assert!(spec(&cache, doc, "other.md", false).is_none()); // markdown exists
    }
}
