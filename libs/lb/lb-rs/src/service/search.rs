use super::activity::RankingWeights;
use crate::logic::file_like::FileLike;
use crate::logic::filename::DocumentType;
use crate::logic::tree_like::TreeLike;
use crate::model::clock;
use crate::model::errors::{LbResult, UnexpectedError};
use crate::Lb;
use futures::stream::{self, FuturesUnordered, StreamExt, TryStreamExt};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sublime_fuzzy::{FuzzySearch, Scoring};
use tokio::sync::RwLock;
use tokio::time::sleep;
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
    pub scheduled_build: Arc<AtomicBool>,
    pub last_built: Arc<AtomicU64>,
    pub docs: Arc<RwLock<Vec<SearchIndexEntry>>>,
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
        if self.search.docs.read().await.is_empty() || !self.config.background_work {
            self.build_index().await?;
        }

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
        results.truncate(20);

        Ok(results)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn build_index(&self) -> LbResult<()> {
        let ts = clock::get_time().0 as u64;
        self.search.last_built.store(ts, Ordering::SeqCst);
        // fetch metadata once; use single lazy tree to re-use key decryption work for all files (lb lock)
        let account = self.get_account()?;
        let mut tree = {
            let tx = self.ro_tx().await;
            let db = tx.db();

            let base = db.base_metadata.get().clone();
            let local = db.local_metadata.get().clone();
            let staged = base.to_staged(local);
            staged.to_lazy()
        };

        // construct replacement index
        let mut replacement_index = Vec::new();
        for id in tree.owned_ids() {
            if tree.calculate_deleted(&id)? {
                continue;
            }
            if tree.in_pending_share(&id)? {
                continue;
            }
            let name = tree.name_using_links(&id, account)?;
            let path = tree.id_to_path(&id, account)?;

            // once per file, re-lock lb to read document using up-to-date hmac (lb lock)
            // because lock has been dropped in the meantime, original `tree` is now arbitrarily out-of-date
            let tx = self.ro_tx().await;
            let db = tx.db();
            let hmac_tree = db.base_metadata.stage(&db.local_metadata); // not lazy bc no decryption uses this one

            let Some(file) = hmac_tree.maybe_find(&id) else { continue }; // file maybe deleted since we started

            let content = if DocumentType::from_file_name_using_extension(&name)
                == DocumentType::Text
            {
                match file.document_hmac() {
                    Some(&hmac) => {
                        let encrypted_content = self.docs.get(id, Some(hmac)).await?;

                        let content = if encrypted_content.value.len() <= CONTENT_MAX_LEN_BYTES {
                            // use the original tree to decrypt the content
                            let decrypted_content =
                                tree.decrypt_document(&id, &encrypted_content, account)?;
                            Some(String::from_utf8_lossy(&decrypted_content).into_owned())
                        } else {
                            None
                        };

                        content
                    }
                    None => None,
                }
            } else {
                None
            };

            replacement_index.push(SearchIndexEntry { id, path, content })
        }

        // swap in replacement index (index lock)
        *self.search.docs.write().await = replacement_index;

        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    /// ensure the index is not built more frequently than every 5s
    pub fn spawn_build_index(&self) {
        if self.config.background_work {
            tokio::spawn({
                let lb = self.clone();
                async move {
                    let ts = clock::get_time().0 as u64;
                    let since_last = ts - lb.search.last_built.load(Ordering::SeqCst);
                    if since_last < 5000 {
                        if lb.search.scheduled_build.load(Ordering::SeqCst) {
                            // index is pretty fresh, and there is a build scheduled in the future, do
                            // nothing
                            return;
                        } else {
                            // wait until about 5s since last build and then try the whole routine
                            // again
                            lb.search.scheduled_build.store(true, Ordering::SeqCst);
                            sleep(Duration::from_millis(5001 - since_last)).await;
                            lb.spawn_build_index();
                        }
                    }

                    // if we make it here there are no scheduled builds reset that flag
                    lb.search.scheduled_build.store(false, Ordering::SeqCst);
                    if let Err(e) = lb.build_index().await {
                        error!("Error building search index: {:?}", e)
                    }
                }
            });
        }
    }
}

impl SearchIndex {
    async fn search_paths(&self, input: &str) -> LbResult<Vec<SearchResult>> {
        let docs_guard = self.docs.read().await; // read lock held for the whole fn

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
        let docs = self.docs.read().await;

        for (idx, _) in docs.iter().enumerate() {
            search_futures.push(async move {
                let doc = &self.docs.read().await[idx];
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
