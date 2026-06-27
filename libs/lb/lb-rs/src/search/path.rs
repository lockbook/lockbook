use super::{SearchFilter, SearchResult, build_descendants};
use crate::Lb;
use crate::model::file::File;
use nucleo::{
    Matcher, Nucleo,
    pattern::{CaseMatching, Normalization},
};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

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
    descendants: HashMap<Uuid, Vec<Uuid>>,
    path_to_id: HashMap<String, Uuid>,
    filter_ids: Option<HashSet<Uuid>>,
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

        let descendants = build_descendants(&files);
        let path_to_id = paths.iter().map(|(id, path)| (path.clone(), *id)).collect();

        let mut searcher = Self {
            nucleo,
            results: Vec::new(),
            submitted_query: String::new(),
            descendants,
            path_to_id,
            filter_ids: None,
        };
        searcher.query("");
        searcher
    }

    /// Update the active filter and refresh results for the current query.
    pub fn update_filter(&mut self, filter: Option<SearchFilter>) {
        self.filter_ids = filter.map(|SearchFilter::Path(path)| {
            self.path_to_id
                .get(&path)
                .and_then(|id| self.descendants.get(id))
                .map(|ids| ids.iter().copied().collect())
                .unwrap_or_default()
        });
        let query = self.submitted_query.clone();
        self.query(&query);
    }

    /// Update the search query. Results available via `results()`.
    pub fn query(&mut self, input: &str) {
        self.nucleo.pattern.reparse(
            0,
            input,
            CaseMatching::Smart,
            Normalization::Smart,
            input.starts_with(&self.submitted_query),
        );
        self.submitted_query = input.to_string();

        while self.nucleo.tick(10).running {}

        // Build results
        self.results.clear();
        let snapshot = self.nucleo.snapshot();

        if input.is_empty() {
            let mut entries: Vec<&PathEntry> = (0..snapshot.matched_item_count())
                .filter_map(|i| snapshot.get_matched_item(i).map(|item| item.data))
                .filter(|e| self.filter_ids.as_ref().map_or(true, |ids| ids.contains(&e.file.id)))
                .collect();
            entries.sort_by_key(|e| Reverse(e.file.last_modified));
            self.results
                .extend(entries.into_iter().take(100).map(|e| SearchResult {
                    id: e.file.id,
                    filename: e.filename.clone(),
                    parent_path: e.parent_path.clone(),
                    is_folder: e.file.is_folder(),
                    path_indices: Vec::new(),
                    path_matches: Vec::new(),
                    content_matches: Vec::new(),
                }));
            return;
        }

        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);

        for i in 0..snapshot.matched_item_count() {
            if self.results.len() >= 100 {
                break;
            }
            if let Some(item) = snapshot.get_matched_item(i) {
                if let Some(ids) = &self.filter_ids {
                    if !ids.contains(&item.data.file.id) {
                        continue;
                    }
                }

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
                    is_folder: item.data.file.is_folder(),
                    path_indices: indices,
                    path_matches: Vec::new(),
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
