use std::sync::{Arc, RwLock};

use lb_rs::Uuid;
use lb_rs::blocking::Lb;

use crate::file_cache::{FileCache, FilesExt as _};

pub use crate::file_cache::ResolvedLink;

/// Visual state for a link, used to color the link text and potentially show
/// a hover tooltip explaining the state.
#[derive(Clone, PartialEq, Eq)]
pub enum LinkState {
    Normal,
    Warning {
        message: String,
    },
    Broken {
        message: String,
    },
    /// A wikilink to a note that doesn't exist yet; opening it creates the note.
    Placeholder {
        message: String,
    },
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

    /// Where opening an unmatched wikilink creates its note: an existing
    /// folder and the names to create under it, the document last.
    fn wikilink_placement(&self, _title: &str) -> Option<(Uuid, Vec<String>)> {
        None
    }

    /// The URL that means file `id` wherever it is pasted.
    fn external_url(&self, _id: Uuid) -> Option<String> {
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

const OUTSIDE_SCOPE_MSG: &str = "Not everyone who can read this note may be able to see this file.";

/// Resolver backed by lockbook's file cache. Resolves links written in a
/// given file within that file's scope; an `lb://` link that leaves the
/// scope is flagged with a yellow warning.
#[derive(Clone)]
pub struct FileCacheLinkResolver {
    files: Arc<RwLock<FileCache>>,
    file_id: Uuid,
    creates_notes: bool,
    core: Option<Lb>,
}

impl FileCacheLinkResolver {
    pub fn new(files: Arc<RwLock<FileCache>>, file_id: Uuid) -> Self {
        Self { files, file_id, creates_notes: false, core: None }
    }

    /// Opening a wikilink nothing matches creates the note it names.
    pub fn creating_notes(self, creates_notes: bool) -> Self {
        Self { creates_notes, ..self }
    }

    /// Links to files copy as their external URLs.
    pub fn copying_external_urls(self, core: Lb) -> Self {
        Self { core: Some(core), ..self }
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
            None => LinkState::Broken { message: "Destination not found".into() },
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
            [] => match guard.wikilink_placement(title, self.file_id) {
                Some((_, names)) if self.creates_notes => LinkState::Placeholder {
                    message: format!("Open to create {}", names.join("/")),
                },
                _ => LinkState::Broken { message: "Destination not found".into() },
            },
            _ => LinkState::Broken {
                message: "More than one file matches; add a folder or an extension".into(),
            },
        }
    }

    fn external_url(&self, id: Uuid) -> Option<String> {
        self.core.as_ref()?.get_file_link_url(id).ok()
    }

    fn wikilink_placement(&self, title: &str) -> Option<(Uuid, Vec<String>)> {
        let guard = self.files.read().unwrap();
        if !self.creates_notes || !guard.wikilink_matches(title, self.file_id).is_empty() {
            return None;
        }
        guard.wikilink_placement(title, self.file_id)
    }
}
