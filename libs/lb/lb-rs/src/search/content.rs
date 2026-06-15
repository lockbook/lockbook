use super::path::split_path;
use super::{ContentMatch, SearchResult};
use crate::blocking::Lb;
use crate::model::file::File;
use std::cmp::Reverse;
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct Document {
    file: File,
    filename: String,
    parent_path: String,
    lowercased_content: String,
}

pub struct ContentSearcher {
    documents: Vec<Document>,
    results: Vec<SearchResult>,
    submitted_query: String,
    build_time: Duration,
}

impl ContentSearcher {
    pub fn new(lb: &Lb) -> Self {
        let start = Instant::now();
        let metas = lb.list_metadatas().unwrap_or_default();
        let paths = Arc::new(lb.list_paths_with_ids(None).unwrap_or_default());

        let md_files: Vec<File> = metas
            .into_iter()
            .filter(|m| m.is_document() && m.name.ends_with(".md"))
            .collect();

        let queue = Arc::new(Mutex::new(md_files));
        let documents = Arc::new(Mutex::new(Vec::<Document>::new()));

        let handles: Vec<_> = (0..thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4))
            .map(|_| {
                let queue = queue.clone();
                let paths = paths.clone();
                let documents = documents.clone();
                let lb = lb.clone();
                thread::spawn(move || {
                    loop {
                        let Some(meta) = queue.lock().unwrap().pop() else {
                            return;
                        };

                        let id = meta.id;
                        let doc = lb
                            .read_document(meta.id, false)
                            .ok()
                            .and_then(|bytes| String::from_utf8(bytes).ok());

                        if let Some(content) = doc {
                            let path = paths
                                .iter()
                                .find(|(i, _)| *i == id)
                                .map(|(_, p)| p.clone())
                                .unwrap_or_default();

                            let (parent, name) = split_path(&path);

                            documents.lock().unwrap().push(Document {
                                file: meta,
                                filename: name.to_string(),
                                parent_path: parent.to_string(),
                                lowercased_content: content.to_lowercase(),
                            });
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let documents = Arc::try_unwrap(documents)
            .ok()
            .expect("all workers joined")
            .into_inner()
            .unwrap();

        Self {
            documents,
            results: Vec::new(),
            submitted_query: String::new(),
            build_time: start.elapsed(),
        }
    }

    /// Update the search query. Results available via `results()`.
    pub fn query(&mut self, input: &str) {
        let query = input.to_lowercase();

        if self.submitted_query == query {
            return;
        }
        self.submitted_query = query.clone();
        self.results.clear();

        if query.is_empty() {
            return;
        }

        let words: Vec<&str> = query.split_whitespace().collect();

        for doc in &self.documents {
            let path = if doc.parent_path == "/" {
                format!("/{}", doc.filename)
            } else {
                format!("{}/{}", doc.parent_path, doc.filename)
            }
            .to_lowercase();

            let mut matched_words = vec![false; words.len()];
            let path_matches = collect_matches(&path, &query, &words, &mut matched_words);
            let content_matches =
                collect_matches(&doc.lowercased_content, &query, &words, &mut matched_words);

            let all_words_matched = matched_words.iter().all(|&m| m);
            let has_match = !path_matches.is_empty() || !content_matches.is_empty();

            if all_words_matched && has_match {
                self.results.push(SearchResult {
                    id: doc.file.id,
                    filename: doc.filename.clone(),
                    parent_path: doc.parent_path.clone(),
                    path_indices: Vec::new(),
                    path_matches,
                    content_matches,
                });
            }
        }

        self.results.sort_by_key(|r| {
            let tier = if !r.path_matches.is_empty() {
                0
            } else if r.content_matches.iter().any(|m| m.exact) {
                1
            } else {
                2
            };
            let path_exact = r.path_matches.iter().filter(|m| m.exact).count();
            let content_exact = r.content_matches.iter().filter(|m| m.exact).count();
            (
                tier,
                Reverse(path_exact),
                Reverse(r.path_matches.len()),
                Reverse(content_exact),
                Reverse(r.content_matches.len()),
            )
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
            .map(|d| d.lowercased_content.as_str())
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
            for (i, &(_, c)) in char_indices
                .iter()
                .enumerate()
                .take(match_start_ci)
                .skip(start)
            {
                if c.is_whitespace() {
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

fn collect_matches(
    haystack: &str, query: &str, words: &[&str], matched_words: &mut [bool],
) -> Vec<ContentMatch> {
    let mut matches = Vec::new();

    for (idx, _) in haystack.match_indices(query) {
        matches.push(ContentMatch { range: idx..idx + query.len(), exact: true });
    }

    for (wi, word) in words.iter().enumerate() {
        for (idx, _) in haystack.match_indices(word) {
            matched_words[wi] = true;
            if matches.iter().any(|m| m.range.contains(&idx)) {
                continue;
            }
            matches.push(ContentMatch { range: idx..idx + word.len(), exact: false });
        }
    }

    matches
}
