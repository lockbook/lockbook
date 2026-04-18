use super::path::split_path;
use super::{ContentMatch, SearchResult};
use crate::blocking::Lb;
use crate::model::file::File;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct Document {
    file: File,
    filename: String,
    parent_path: String,
    content: String, // lowercased
}

pub struct ContentSearcher {
    documents: Vec<Document>,
    results: Vec<SearchResult>,
    submitted_query: String,
    build_time: Duration,
}

impl ContentSearcher {
    pub async fn new(lb: &Lb) -> Self {
        let start = Instant::now();
        let metas = lb.list_metadatas().await.unwrap_or_default();
        let paths = lb.list_paths_with_ids(None).await.unwrap_or_default();

        let md_files: Vec<File> = metas
            .into_iter()
            .filter(|m| m.is_document() && m.name.ends_with(".md"))
            .collect();

        let futures = md_files.into_iter().map(|meta| async {
            let id = meta.id;
            let doc = lb
                .read_document(id, false)
                .await
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok());
            (meta, doc)
        });

        let mut documents = Vec::new();
        let mut stream = stream::iter(futures).buffer_unordered(4);

        while let Some((meta, doc)) = stream.next().await {
            let Some(content) = doc else { continue };

            let path = paths
                .iter()
                .find(|(i, _)| *i == meta.id)
                .map(|(_, p)| p.clone())
                .unwrap_or_default();

            let (parent, name) = split_path(&path);

            documents.push(Document {
                file: meta,
                filename: name.to_string(),
                parent_path: parent.to_string(),
                content: content.to_lowercase(),
            });
        }

        Self {
            documents,
            results: Vec::new(),
            submitted_query: String::new(),
            build_time: start.elapsed(),
        }
    }

    /// Update the search query. Results available via `results()`.
    pub fn query(&mut self, input: &str) {
        let query = input.to_ascii_lowercase();

        if self.submitted_query == query {
            return;
        }
        self.submitted_query = query.clone();
        self.results.clear();

        if query.is_empty() {
            return;
        }

        for doc in &self.documents {
            let mut content_matches = Vec::new();

            // Find exact matches
            for (idx, _) in doc.content.match_indices(&query) {
                content_matches.push(ContentMatch { range: idx..idx + query.len(), exact: true });
            }

            // Find word matches
            let mut all_words_matched = true;
            for word in query.split_whitespace() {
                let mut word_matched = false;
                for (idx, _) in doc.content.match_indices(word) {
                    word_matched = true;
                    if content_matches.iter().any(|h| h.range.contains(&idx)) {
                        continue;
                    }
                    content_matches
                        .push(ContentMatch { range: idx..idx + word.len(), exact: false });
                }
                if !word_matched {
                    all_words_matched = false;
                }
            }

            if all_words_matched && !content_matches.is_empty() {
                self.results.push(SearchResult {
                    id: doc.file.id,
                    filename: doc.filename.clone(),
                    parent_path: doc.parent_path.clone(),
                    path_indices: Vec::new(),
                    content_matches,
                });
            }
        }

        self.results.sort_unstable_by(|a, b| {
            let a_exact = a.content_matches.iter().filter(|m| m.exact).count();
            let b_exact = b.content_matches.iter().filter(|m| m.exact).count();
            if a_exact > 0 || b_exact > 0 {
                b_exact.cmp(&a_exact)
            } else {
                b.content_matches.len().cmp(&a.content_matches.len())
            }
        });
    }

    /// Get search results.
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }

    /// How long it took to build the search index.
    pub fn build_time(&self) -> Duration {
        self.build_time
    }

    /// Get document content by ID.
    pub fn content(&self, id: Uuid) -> Option<&str> {
        self.documents
            .iter()
            .find(|d| d.file.id == id)
            .map(|d| d.content.as_str())
    }

    /// Extract snippet with context. Returns (prefix, matched, suffix).
    pub fn snippet(
        &self, id: Uuid, range: &Range<usize>, context_chars: usize,
    ) -> Option<(&str, &str, &str)> {
        let content = self.content(id)?;
        let char_indices: Vec<(usize, char)> = content.char_indices().collect();

        let match_start_ci = char_indices
            .iter()
            .position(|(byte, _)| *byte >= range.start)
            .unwrap_or(char_indices.len());
        let match_end_ci = char_indices
            .iter()
            .position(|(byte, _)| *byte >= range.end)
            .unwrap_or(char_indices.len());

        let start_ci = match_start_ci.saturating_sub(context_chars);
        let end_ci = (match_end_ci + context_chars).min(char_indices.len());

        // Snap to whitespace
        let mut start = start_ci;
        if start > 0 {
            for i in start..match_start_ci {
                if char_indices[i].1.is_whitespace() {
                    start = i + 1;
                    break;
                }
            }
        }

        let mut end = end_ci;
        if end < char_indices.len() {
            for i in (match_end_ci..end).rev() {
                if char_indices[i].1.is_whitespace() {
                    end = i;
                    break;
                }
            }
        }

        let get_byte = |ci: usize| {
            char_indices
                .get(ci)
                .map(|(b, _)| *b)
                .unwrap_or(content.len())
        };

        let start_byte = get_byte(start);
        let match_start_byte = get_byte(match_start_ci);
        let match_end_byte = get_byte(match_end_ci);
        let end_byte = get_byte(end);

        Some((
            &content[start_byte..match_start_byte],
            &content[match_start_byte..match_end_byte],
            &content[match_end_byte..end_byte],
        ))
    }
}
