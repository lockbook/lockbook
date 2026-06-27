pub mod content;
pub mod path;

use std::collections::HashMap;
use std::ops::Range;
use uuid::Uuid;

use crate::model::file::File;

pub use content::ContentSearcher;
pub use path::PathSearcher;

/// Map each file id to all of its descendant ids (children, grandchildren, ...).
/// Built once when an executor's index is constructed.
pub(crate) fn build_descendants(files: &[File]) -> HashMap<Uuid, Vec<Uuid>> {
    let parent_of: HashMap<Uuid, Uuid> = files.iter().map(|f| (f.id, f.parent)).collect();

    let mut descendants: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for f in files {
        let mut node = f.id;
        while let Some(&parent) = parent_of.get(&node) {
            if parent == node {
                break; // root is its own parent
            }
            descendants.entry(parent).or_default().push(f.id);
            node = parent;
        }
    }

    descendants
}

/// Unified search result for both path and content searches.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub id: Uuid,
    pub filename: String,
    pub parent_path: String,
    pub is_folder: bool,
    /// Character indices in the full path that matched (for path search highlighting).
    pub path_indices: Vec<u32>,
    pub path_matches: Vec<ContentMatch>,
    /// Content matches within the document.
    pub content_matches: Vec<ContentMatch>,
}

/// Scopes a search to a subset of the working set.
#[derive(Debug, Clone)]
pub enum SearchFilter {
    /// Restrict results to a folder path.
    Path(String),
}

/// A match highlight within document content.
#[derive(Debug, Clone)]
pub struct ContentMatch {
    /// Byte range into the document content.
    pub range: Range<usize>,
    /// Whether this is an exact match of the full query.
    pub exact: bool,
}

use crate::Lb;

impl Lb {
    pub async fn path_searcher(&self) -> PathSearcher {
        PathSearcher::new(self).await
    }
}
