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
    name_lower: String,
    stem_lower: String,
    path_lower: String,
}

/// 0: the name, with or without its extension, is the query. 1: the name starts with
/// the query. 2: every query word is in the name. 3: every word is somewhere in the
/// path. 4: a looser fuzzy match.
fn match_tier(entry: &PathEntry, query: &str, words: &[String]) -> u8 {
    if entry.stem_lower == query || entry.name_lower == query {
        0
    } else if entry.name_lower.starts_with(query) {
        1
    } else if words.iter().all(|w| entry.name_lower.contains(w)) {
        2
    } else if words.iter().all(|w| entry.path_lower.contains(w)) {
        3
    } else {
        4
    }
}

fn path_result(entry: &PathEntry, path_indices: Vec<u32>) -> SearchResult {
    SearchResult {
        id: entry.file.id,
        filename: entry.filename.clone(),
        parent_path: entry.parent_path.clone(),
        is_folder: entry.file.is_folder(),
        path_indices,
        path_matches: Vec::new(),
        content_matches: Vec::new(),
    }
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
        let paths = lb.list_paths_with_ids(None).await.unwrap_or_default();
        let by_id: HashMap<Uuid, File> = files.into_iter().map(|f| (f.id, f)).collect();
        Self::from_files(
            paths
                .into_iter()
                .filter_map(|(id, path)| by_id.get(&id).cloned().map(|file| (file, path))),
        )
    }

    /// Build a path index from an in-memory file list (`File` + path string).
    /// Same matcher as [`Self::new`]; used by editor link completions so they
    /// share ranking (including empty-query recents) with filename search.
    pub fn from_files(entries: impl IntoIterator<Item = (File, String)>) -> Self {
        let notify = Arc::new(|| {});
        let nucleo: Nucleo<PathEntry> = Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1);
        let injector = nucleo.injector();

        let mut files = Vec::new();
        let mut path_to_id = HashMap::new();

        for (file, path) in entries {
            if path == "/" {
                files.push(file);
                continue;
            }
            let (parent_path, filename) = split_path(&path);
            injector.push(
                PathEntry {
                    file: file.clone(),
                    path: path.clone(),
                    filename: filename.to_string(),
                    parent_path: parent_path.to_string(),
                    name_lower: filename.to_lowercase(),
                    stem_lower: match file.is_folder() {
                        true => filename,
                        false => filename.rsplit_once('.').map_or(filename, |(stem, _)| stem),
                    }
                    .to_lowercase(),
                    path_lower: path.to_lowercase(),
                },
                |entry, cols| {
                    cols[0] = entry.path.as_str().into();
                },
            );
            path_to_id.insert(path, file.id);
            files.push(file);
        }

        let descendants = build_descendants(&files);

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
        self.filter_ids = super::resolve_filter(filter, &self.path_to_id, &self.descendants);
        self.rebuild();
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

        self.rebuild();
    }

    /// Rebuild `results` from the current query and filter without re-running the
    /// matcher. Shared by `query` (after a reparse) and `update_filter`.
    fn rebuild(&mut self) {
        self.results.clear();
        let snapshot = self.nucleo.snapshot();

        if self.submitted_query.is_empty() {
            let mut entries: Vec<&PathEntry> = (0..snapshot.matched_item_count())
                .filter_map(|i| snapshot.get_matched_item(i).map(|item| item.data))
                .filter(|e| {
                    self.filter_ids
                        .as_ref()
                        .is_none_or(|ids| ids.contains(&e.file.id))
                })
                .collect();
            entries.sort_by_key(|e| Reverse(e.file.last_modified));
            self.results.extend(
                entries
                    .into_iter()
                    .take(100)
                    .map(|e| path_result(e, Vec::new())),
            );
            return;
        }

        let words: Vec<String> = self
            .submitted_query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let query = words.join(" ");
        let mut ranked: Vec<_> = (0..snapshot.matched_item_count())
            .filter_map(|i| snapshot.get_matched_item(i))
            .filter(|item| {
                self.filter_ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&item.data.file.id))
            })
            .collect();
        // Stable, so loose matches keep nucleo's score order.
        ranked.sort_by_cached_key(|item| {
            let tier = match_tier(item.data, &query, &words);
            let recency = if tier < 4 { item.data.file.last_modified } else { 0 };
            (tier, Reverse(recency))
        });

        let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
        let pattern = self.nucleo.pattern.column_pattern(0);
        self.results
            .extend(ranked.into_iter().take(100).map(|item| {
                let mut indices = Vec::new();
                pattern.indices(item.matcher_columns[0].slice(..), &mut matcher, &mut indices);
                path_result(item.data, indices)
            }));
    }

    /// Get search results.
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }
}

#[cfg(test)]
mod tests {
    use super::PathSearcher;
    use crate::model::file::File;
    use crate::model::file_metadata::FileType;
    use uuid::Uuid;

    fn entry(path: &str, last_modified: u64) -> (File, String) {
        let file_type = if path.ends_with('/') { FileType::Folder } else { FileType::Document };
        let file = File {
            id: Uuid::new_v4(),
            parent: Uuid::nil(),
            name: path.to_string(),
            file_type,
            last_modified,
            last_modified_by: String::new(),
            owner: String::new(),
            shares: Vec::new(),
            size_bytes: 0,
        };
        (file, path.to_string())
    }

    #[test]
    fn exact_then_prefix_then_name_then_path_newest_first() {
        let mut searcher = PathSearcher::from_files([
            entry("/lb-docs/release.md", 1),
            entry("/a/b/c/d/release-26.11.5.md", 2),
            entry("/a/b/c/d/release-26.11.12.md", 3),
            entry("/release-ops/pending-release.md", 4),
            entry("/release-ops/qa.md", 5),
        ]);
        searcher.query("release");
        let names: Vec<_> = searcher
            .results()
            .iter()
            .map(|r| r.filename.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "release.md",
                "release-26.11.12.md",
                "release-26.11.5.md",
                "pending-release.md",
                "qa.md"
            ]
        );
    }
}
