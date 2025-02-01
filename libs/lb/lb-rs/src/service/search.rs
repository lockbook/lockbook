use super::activity::RankingWeights;
use super::events::Event;
use crate::model::filename::DocumentType;
use crate::model::errors::{LbErr, LbErrKind, LbResult, UnexpectedError};
use crate::Lb;
use futures::stream::{self, FuturesUnordered, StreamExt, TryStreamExt};
use serde::Serialize;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use sublime_fuzzy::{FuzzySearch, Scoring};
use tokio::sync::RwLock;
use uuid::Uuid;

const CONTENT_SCORE_THRESHOLD: i64 = 170;
const PATH_SCORE_THRESHOLD: i64 = 10;
const CONTENT_MAX_LEN_BYTES: usize = 128 * 1024; // 128kb

const MAX_CONTENT_MATCH_LENGTH: usize = 400;
const IDEAL_CONTENT_MATCH_LENGTH: usize = 150;
const CONTENT_MATCH_PADDING: usize = 8;

const FUZZY_WEIGHT: f32 = 0.8;

#[derive(Clone, Default)]
pub struct SearchIndex {
    pub building_index: Arc<AtomicBool>,
    pub index: Arc<RwLock<Vec<SearchIndexEntry>>>,
}

#[derive(Debug)]
pub struct SearchIndexEntry {
    pub id: Uuid,
    pub path: String,
    pub content: Option<String>,
}

#[derive(Copy, Clone, Debug)]
pub enum SearchConfig {
    Paths,
    Documents,
    PathsAndDocuments,
}

#[derive(Debug)]
pub enum SearchResult {
    DocumentMatch { id: Uuid, path: String, content_matches: Vec<ContentMatch> },
    PathMatch { id: Uuid, path: String, matched_indices: Vec<usize>, score: i64 },
}

impl SearchResult {
    pub fn id(&self) -> Uuid {
        match self {
            SearchResult::DocumentMatch { id, .. } | SearchResult::PathMatch { id, .. } => *id,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            SearchResult::DocumentMatch { path, .. } | SearchResult::PathMatch { path, .. } => path,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            SearchResult::DocumentMatch { path, .. } | SearchResult::PathMatch { path, .. } => {
                path.split('/').last().unwrap_or_default()
            }
        }
    }

    pub fn score(&self) -> i64 {
        match self {
            SearchResult::DocumentMatch { content_matches, .. } => content_matches
                .iter()
                .map(|m| m.score)
                .max()
                .unwrap_or_default(),
            SearchResult::PathMatch { score, .. } => *score,
        }
    }
}

impl Lb {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        // for cli style invocations nothing will have built the search index yet
        if !self.config.background_work {
            self.build_index().await?;
        }

        // show suggested docs if the input string is empty
        if input.is_empty() {
            match cfg {
                SearchConfig::Paths | SearchConfig::PathsAndDocuments => {
                    return stream::iter(self.suggested_docs(RankingWeights::default()).await?)
                        .then(|id| async move {
                            Ok(SearchResult::PathMatch {
                                id,
                                path: self.get_path_by_id(id).await?,
                                matched_indices: vec![],
                                score: 0,
                            })
                        })
                        .try_collect()
                        .await;
                }
                SearchConfig::Documents => return Ok(vec![]),
            }
        }

        // if the index is empty wait patiently for it become available
        let mut retries = 0;
        loop {
            if self.search.index.read().await.is_empty() {
                warn!("search index was empty, waiting 50ms");
                tokio::time::sleep(Duration::from_millis(50)).await;
                retries += 1;

                if retries == 20 {
                    error!("could not aquire search index after 20x(50ms) retries.");
                    return Err(LbErr::from(LbErrKind::Unexpected(
                        "failed to search, index not available".to_string(),
                    )));
                }
            } else {
                break;
            }
        }

        let mut results = match cfg {
            SearchConfig::Paths => self.search.search_paths(input).await?,
            SearchConfig::Documents => self.search.search_content(input).await?,
            SearchConfig::PathsAndDocuments => {
                let (paths, docs) = tokio::join!(
                    self.search.search_paths(input),
                    self.search.search_content(input)
                );
                paths?.into_iter().chain(docs?.into_iter()).collect()
            }
        };

        results.sort_unstable_by_key(|r| -r.score());
        results.truncate(10);

        Ok(results)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn build_index(&self) -> LbResult<()> {
        // if we haven't signed in yet, we'll leave our index entry and our event subscriber will
        // handle the state change
        if self.keychain.get_account().is_err() {
            return Ok(());
        }

        // some other caller has already built this index, subscriber will keep it up to date
        if self.search.building_index.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let mut tasks = vec![];
        for file in self.list_metadatas().await? {
            let id = file.id;
            let is_doc_searchable =
                DocumentType::from_file_name_using_extension(&file.name) == DocumentType::Text;

            tasks.push(async move {
                let (path, content) = if is_doc_searchable {
                    let (path, doc) =
                        tokio::join!(self.get_path_by_id(id), self.read_document(id, false));

                    let path = path?;

                    let doc = doc?;
                    let doc = if doc.len() >= CONTENT_MAX_LEN_BYTES {
                        None
                    } else {
                        Some(String::from_utf8_lossy(&doc).to_string())
                    };

                    (path, doc)
                } else {
                    (self.get_path_by_id(id).await?, None)
                };

                Ok::<SearchIndexEntry, LbErr>(SearchIndexEntry { id, path, content })
            });
        }

        let mut results = stream::iter(tasks).buffer_unordered(
            thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .into(),
        );
        let mut replacement_index = vec![];
        while let Some(res) = results.next().await {
            replacement_index.push(res?);
        }

        // swap in replacement index (index lock)
        *self.search.index.write().await = replacement_index;

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    pub fn setup_search(&self) {
        if self.config.background_work {
            let lb = self.clone();
            let mut rx = self.subscribe();
            tokio::spawn(async move {
                lb.build_index().await.unwrap();
                loop {
                    let evt = match rx.recv().await {
                        Ok(evt) => evt,
                        Err(err) => {
                            error!("failed to receive from a channel {err}");
                            return;
                        }
                    };

                    match evt {
                        Event::MetadataChanged(mut id) => {
                            // if this file is deleted recompute all our metadata
                            if lb.get_file_by_id(id).await.is_err() {
                                id = lb.root().await.unwrap().id;
                            }

                            // compute info for this update up-front
                            let files = lb.list_metadatas().await.unwrap();
                            let all_file_ids: Vec<Uuid> = files.into_iter().map(|f| f.id).collect();
                            let children = lb.get_and_get_children_recursively(&id).await.unwrap();
                            let mut paths = HashMap::new();
                            for child in children {
                                // todo: ideally this would be a single efficient core call
                                paths.insert(child.id, lb.get_path_by_id(child.id).await.unwrap());
                            }

                            // aquire the lock
                            let mut index = lb.search.index.write().await;

                            // handle deletions
                            index.retain(|entry| all_file_ids.contains(&entry.id));

                            // update any of the paths of this file and the children
                            for entry in index.iter_mut() {
                                if paths.contains_key(&entry.id) {
                                    entry.path = paths.remove(&entry.id).unwrap();
                                }
                            }

                            // handle any remaining, new metadata
                            for (id, path) in paths {
                                // any content should come in as a result of DocumentWritten
                                index.push(SearchIndexEntry { id, path, content: None });
                            }
                        }

                        Event::DocumentWritten(id) => {
                            let file = lb.get_file_by_id(id).await.unwrap();
                            let is_searchable =
                                DocumentType::from_file_name_using_extension(&file.name)
                                    == DocumentType::Text;

                            let doc = lb.read_document(id, false).await.unwrap();
                            let doc = if doc.len() >= CONTENT_MAX_LEN_BYTES || !is_searchable {
                                None
                            } else {
                                Some(String::from_utf8_lossy(&doc).to_string())
                            };

                            let mut index = lb.search.index.write().await;
                            let mut found = false;
                            // todo: consider warn! when doc not found
                            for entries in index.iter_mut() {
                                if entries.id == id {
                                    entries.content = doc;
                                    found = true;
                                    break;
                                }
                            }

                            if !found {
                                warn!("could {file:?} not insert doc into index");
                            }
                        }
                    };
                }
            });
        }
    }
}

impl SearchIndex {
    async fn search_paths(&self, input: &str) -> LbResult<Vec<SearchResult>> {
        let docs_guard = self.index.read().await; // read lock held for the whole fn

        let mut results = Vec::new();
        for doc in docs_guard.iter() {
            if let Some(p_match) = FuzzySearch::new(input, &doc.path)
                .case_insensitive()
                .score_with(&Scoring::emphasize_distance())
                .best_match()
            {
                let score = (p_match.score().min(600) as f32 * FUZZY_WEIGHT) as i64;

                if score > PATH_SCORE_THRESHOLD {
                    results.push(SearchResult::PathMatch {
                        id: doc.id,
                        path: doc.path.clone(),
                        matched_indices: p_match.matched_indices().cloned().collect(),
                        score,
                    });
                }
            }
        }
        Ok(results)
    }

    async fn search_content(&self, input: &str) -> LbResult<Vec<SearchResult>> {
        let search_futures = FuturesUnordered::new();
        let docs = self.index.read().await;

        for (idx, _) in docs.iter().enumerate() {
            search_futures.push(async move {
                let doc = &self.index.read().await[idx];
                let id = doc.id;
                let path = &doc.path;
                let content = &doc.content;
                if let Some(content) = content {
                    let mut content_matches = Vec::new();

                    for paragraph in content.split("\n\n") {
                        if let Some(c_match) = FuzzySearch::new(input, paragraph)
                            .case_insensitive()
                            .score_with(&Scoring::emphasize_distance())
                            .best_match()
                        {
                            let score = (c_match.score().min(600) as f32 * FUZZY_WEIGHT) as i64;
                            let (paragraph, matched_indices) = match Self::optimize_searched_text(
                                paragraph,
                                c_match.matched_indices().cloned().collect(),
                            ) {
                                Ok((paragraph, matched_indices)) => (paragraph, matched_indices),
                                Err(_) => continue,
                            };

                            if score > CONTENT_SCORE_THRESHOLD {
                                content_matches.push(ContentMatch {
                                    paragraph,
                                    matched_indices,
                                    score,
                                });
                            }
                        }
                    }

                    if !content_matches.is_empty() {
                        return Some(SearchResult::DocumentMatch {
                            id,
                            path: path.clone(),
                            content_matches,
                        });
                    }
                }
                None
            });
        }

        Ok(search_futures
            .collect::<Vec<Option<SearchResult>>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<SearchResult>>())
    }

    fn optimize_searched_text(
        paragraph: &str, matched_indices: Vec<usize>,
    ) -> Result<(String, Vec<usize>), UnexpectedError> {
        if paragraph.len() <= IDEAL_CONTENT_MATCH_LENGTH {
            return Ok((paragraph.to_string(), matched_indices));
        }

        let mut index_offset: usize = 0;
        let mut new_paragraph = paragraph.to_string();
        let mut new_indices = matched_indices;

        let first_match = new_indices.first().ok_or_else(|| {
            warn!("A fuzzy match happened but there are no matched indices.");
            UnexpectedError::new("No matched indices.".to_string())
        })?;

        let last_match = new_indices.last().ok_or_else(|| {
            warn!("A fuzzy match happened but there are no matched indices.");
            UnexpectedError::new("No matched indices.".to_string())
        })?;

        if *last_match < IDEAL_CONTENT_MATCH_LENGTH {
            new_paragraph = new_paragraph
                .chars()
                .take(IDEAL_CONTENT_MATCH_LENGTH + CONTENT_MATCH_PADDING)
                .chain("...".chars())
                .collect();
        } else {
            if *first_match > CONTENT_MATCH_PADDING {
                let at_least_take = new_paragraph.len() - first_match + CONTENT_MATCH_PADDING;

                let deleted_chars_len = if at_least_take > IDEAL_CONTENT_MATCH_LENGTH {
                    first_match - CONTENT_MATCH_PADDING
                } else {
                    new_paragraph.len() - IDEAL_CONTENT_MATCH_LENGTH
                };

                index_offset = deleted_chars_len - 3;

                new_paragraph = "..."
                    .chars()
                    .chain(new_paragraph.chars().skip(deleted_chars_len))
                    .collect();
            }

            if new_paragraph.len() > IDEAL_CONTENT_MATCH_LENGTH + CONTENT_MATCH_PADDING + 3 {
                let at_least_take = *last_match - index_offset + CONTENT_MATCH_PADDING;

                let take_chars_len = if at_least_take > IDEAL_CONTENT_MATCH_LENGTH {
                    at_least_take
                } else {
                    IDEAL_CONTENT_MATCH_LENGTH
                };

                new_paragraph = new_paragraph
                    .chars()
                    .take(take_chars_len)
                    .chain("...".chars())
                    .collect();
            }

            if new_paragraph.len() > MAX_CONTENT_MATCH_LENGTH {
                new_paragraph = new_paragraph
                    .chars()
                    .take(MAX_CONTENT_MATCH_LENGTH)
                    .chain("...".chars())
                    .collect();

                new_indices.retain(|index| (*index - index_offset) < MAX_CONTENT_MATCH_LENGTH)
            }
        }

        Ok((
            new_paragraph,
            new_indices
                .iter()
                .map(|index| *index - index_offset)
                .collect(),
        ))
    }
}

#[derive(Debug, Serialize)]
pub struct ContentMatch {
    pub paragraph: String,
    pub matched_indices: Vec<usize>,
    pub score: i64,
}
