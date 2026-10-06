use std::sync::{Arc, RwLock};

use lb_rs::Uuid;

use crate::file_cache::{FileCache, FilesExt as _, link_id};

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

const OUTSIDE_SCOPE_MSG: &str = "Not everyone who can read this note may be able to see this file.";
const BEYOND_SCOPE_MSG: &str = "This file is outside the folder this note is shared in.";
const BEYOND_REACH_MSG: &str = "This note's owner can't see this file.";
const NOT_FOUND_MSG: &str = "Destination not found";

/// Resolver backed by lockbook's file cache. Resolves links written in a
/// given file within that file's scope; an `lb://` link that leaves the
/// scope is flagged with a yellow warning.
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
        let guard = self.files.read().unwrap();
        guard.resolve_wikilink(title, self.file_id)
    }

    fn link_state(&self, url: &str) -> LinkState {
        let guard = self.files.read().unwrap();
        match guard.resolve_link(url, self.file_id) {
            None => {
                let named = link_id(url).and_then(|id| guard.get_by_id(id));
                let message = if guard.beyond_scope(url, false, self.file_id).is_some() {
                    BEYOND_SCOPE_MSG
                } else if named.is_some_and(|file| file.is_document()) {
                    BEYOND_REACH_MSG
                } else {
                    NOT_FOUND_MSG
                };
                LinkState::Broken { message: message.into() }
            }
            Some(ResolvedLink::External(_)) => LinkState::Normal,
            Some(ResolvedLink::File(target)) => {
                if guard.in_scope(guard.scope_top(self.file_id), target) {
                    LinkState::Normal
                } else {
                    LinkState::Warning { message: OUTSIDE_SCOPE_MSG.into() }
                }
            }
        }
    }

    fn wikilink_state(&self, title: &str) -> LinkState {
        let guard = self.files.read().unwrap();
        match guard.wikilink_matches(title, self.file_id)[..] {
            [_] => LinkState::Normal,
            [] if guard.beyond_scope(title, true, self.file_id).is_some() => {
                LinkState::Broken { message: BEYOND_SCOPE_MSG.into() }
            }
            [] => LinkState::Broken { message: NOT_FOUND_MSG.into() },
            _ => LinkState::Broken {
                message: "More than one file matches; add a folder or an extension".into(),
            },
        }
    }
}
