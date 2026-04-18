use crate::Lb;
use crate::model::file::File;
use super::SearchResult;
use nucleo::{Matcher, Nucleo, pattern::{CaseMatching, Normalization}};
use std::sync::Arc;

/// Split a path into (parent_path, filename).
pub(crate) fn split_path(path: &str) -> (&str, &str) {
    // Strip trailing slash for folders
    let path = path.trim_end_matches('/');
    match path.rfind('/') {
        Some(idx) if idx > 0 => (&path[..idx], &path[idx + 1..]),
        Some(_) => ("/", &path[1..]), // root case: "/filename"
        None => ("", path),
    }
}

#[derive(Clone)]
struct PathEntry {
    file: File,
    path: String,
    filename: String,
    parent_path: String,
}

pub struct PathSearcher {
    nucleo: Nucleo<PathEntry>,
    results: Vec<SearchResult>,
    submitted_query: String,
}

impl PathSearcher {
    pub async fn new(lb: &Lb) -> Self {
        let files = lb.list_metadatas().await.unwrap_or_default();
        let mut paths = lb.list_paths_with_ids(None).await.unwrap_or_default();
        paths.retain(|(_, path)| path != "/");

        let notify = Arc::new(|| {});
        let nucleo: Nucleo<PathEntry> = Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1);
        let injector = nucleo.injector();

        for (id, path) in &paths {
            if let Some(file) = files.iter().find(|f| f.id == *id) {
                let (parent_path, filename) = split_path(path);
                injector.push(
                    PathEntry {
                        file: file.clone(),
                        path: path.clone(),
                        filename: filename.to_string(),
                        parent_path: parent_path.to_string(),
                    },
                    |entry, cols| {
                        cols[0] = entry.path.as_str().into();
                    },
                );
            }
        }

        Self {
            nucleo,
            results: Vec::new(),
            submitted_query: String::new(),
        }
    }

    /// Update the search query. Results available via `results()`.
    pub fn query(&mut self, input: &str) {
        self.nucleo.pattern.reparse(
            0,
            input,
            CaseMatching::Smart,
            Normalization::Smart,
            self.submitted_query.starts_with(input),
        );
        self.submitted_query = input.to_string();

        while self.nucleo.tick(10).running {}

        // Build results
        self.results.clear();
        let snapshot = self.nucleo.snapshot();
        let count = snapshot.matched_item_count().min(100) as u32;
        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);

        for i in 0..count {
            if let Some(item) = snapshot.get_matched_item(i) {
                let mut indices = Vec::new();
                self.nucleo.pattern.column_pattern(0).indices(
                    item.matcher_columns[0].slice(..),
                    &mut matcher,
                    &mut indices,
                );

                self.results.push(SearchResult {
                    id: item.data.file.id,
                    filename: item.data.filename.clone(),
                    parent_path: item.data.parent_path.clone(),
                    path_indices: indices,
                    content_matches: Vec::new(),
                });
            }
        }
    }

    /// Get search results.
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }
}
