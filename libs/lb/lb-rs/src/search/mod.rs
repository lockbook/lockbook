pub mod content;
pub mod path;

use std::ops::Range;
use uuid::Uuid;

pub use content::ContentSearcher;
pub use path::PathSearcher;

/// Unified search result for both path and content searches.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub id: Uuid,
    pub filename: String,
    pub parent_path: String,
    /// Character indices in the full path that matched (for path search highlighting).
    pub path_indices: Vec<u32>,
    /// Content matches within the document.
    pub content_matches: Vec<ContentMatch>,
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
