use std::sync::{Arc, RwLock};

use lb_rs::Uuid;

use crate::file_cache::{FileCache, FilesExt as _};

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
}

impl LinkResolver for FileCacheLinkResolver {
    fn resolve_link(&self, url: &str) -> Option<ResolvedLink> {
        let guard = self.files.read().unwrap();
        let from_id = guard.get_by_id(self.file_id)?.parent;
        guard.resolve_link(url, from_id)
    }

    fn resolve_wikilink(&self, title: &str) -> Option<Uuid> {
        let guard = self.files.read().unwrap();
        let from_id = guard.get_by_id(self.file_id)?.parent;
        guard.resolve_wikilink(title, from_id)
    }

    fn link_state(&self, url: &str) -> LinkState {
        let guard = self.files.read().unwrap();
        let Some(from_id) = guard.get_by_id(self.file_id).map(|f| f.parent) else {
            return LinkState::Broken { message: "Destination not found".into() };
        };
        match guard.resolve_link(url, from_id) {
            None => LinkState::Broken { message: "Destination not found".into() },
            Some(ResolvedLink::External(_)) => LinkState::Normal,
            Some(ResolvedLink::File(target_id)) => {
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
        let guard = self.files.read().unwrap();
        let Some(from_id) = guard.get_by_id(self.file_id).map(|f| f.parent) else {
            return LinkState::Broken { message: "Destination not found".into() };
        };
        match guard.resolve_wikilink(title, from_id) {
            None => LinkState::Broken { message: "Destination not found".into() },
            Some(_) => LinkState::Normal, // wikilinks are always within-tree
        }
    }
}
