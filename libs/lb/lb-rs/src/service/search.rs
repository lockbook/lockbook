use crate::logic::crypto::{DecryptedDocument, EncryptedDocument};
use crate::logic::file_like::FileLike;
use crate::logic::filename::DocumentType;
use crate::logic::tree_like::TreeLike;
use crate::model::errors::{LbErr, LbResult, UnexpectedError};
use crate::model::file::File;
use crate::{Lb};
use crossbeam::channel::{self, Receiver, Sender};
use futures::stream::{self, FuturesUnordered, StreamExt, TryStreamExt};

use futures::{future};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::{self, available_parallelism};
use std::time::Duration;
use sublime_fuzzy::{FuzzySearch, Scoring};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use uuid::Uuid;

use super::activity::{RankingWeights, Stats};

const CONTENT_SCORE_THRESHOLD: i64 = 170;
const PATH_SCORE_THRESHOLD: i64 = 10;

const MAX_CONTENT_MATCH_LENGTH: usize = 400;
const IDEAL_CONTENT_MATCH_LENGTH: usize = 150;
const CONTENT_MATCH_PADDING: usize = 8;

const FUZZY_WEIGHT: f32 = 0.8;

#[derive(Clone, Default)]
pub struct SearchIndex {
    pub is_building_index: Arc<AtomicBool>,
    pub index_built_at: u128,
    pub docs: Arc<RwLock<Vec<Arc<SearchIndexEntry>>>>,
}

pub struct SearchIndexEntry {
    pub id: Uuid,
    pub path: String,
    pub content: Option<String>,
}

#[derive(Copy, Clone)]
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

impl Lb {
    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        if input == ""
            && self
                .search
                .is_building_index
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
        {
            let lb = self.clone();

            tokio::spawn(async move {
                lb.build_index().await.unwrap(); // TODO: remove unwrap
                lb.search.is_building_index.store(false, std::sync::atomic::Ordering::SeqCst);
            });

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

        while self
            .search
            .is_building_index
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            sleep(Duration::from_millis(100)).await;
        }

        match cfg {
            SearchConfig::Paths => Self::search_paths(&self.search.docs, input).await,
            SearchConfig::Documents => Self::search_content(&self.search.docs, input).await,
            SearchConfig::PathsAndDocuments => {
                let docs = &self.search.docs;
                let mut results = Self::search_paths(docs, input).await?;

                results.extend(Self::search_content(docs, input).await?);

                Ok(results)
            }
        }
    }

    async fn search_paths(
        docs: &Arc<RwLock<Vec<Arc<SearchIndexEntry>>>>, input: &str,
    ) -> LbResult<Vec<SearchResult>> {
        let search_futures = FuturesUnordered::new();

        for doc in docs.read().await.iter() {
            let doc = doc.clone();

            search_futures.push(async move {
                if let Some(p_match) = FuzzySearch::new(input, &doc.path)
                    .case_insensitive()
                    .score_with(&Scoring::emphasize_distance())
                    .best_match()
                {
                    let score = (p_match.score().min(600) as f32 * FUZZY_WEIGHT) as i64;

                    if score > PATH_SCORE_THRESHOLD {
                        return Some(SearchResult::PathMatch {
                            id: doc.id,
                            path: doc.path.clone(),
                            matched_indices: p_match.matched_indices().cloned().collect(),
                            score,
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
            .filter_map(|res| res)
            .collect::<Vec<SearchResult>>())
    }

    async fn search_content(
        docs: &Arc<RwLock<Vec<Arc<SearchIndexEntry>>>>, input: &str,
    ) -> LbResult<Vec<SearchResult>> {
        let search_futures = FuturesUnordered::new();

        for doc in docs.read().await.iter() {
            let doc = doc.clone();

            search_futures.push(async move {
                if let Some(content) = &doc.content {
                    let mut sub_results = Vec::new();

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
                                sub_results.push(ContentMatch {
                                    paragraph,
                                    matched_indices,
                                    score,
                                });
                            }
                        }
                    }

                    if !sub_results.is_empty() {
                        return Some(SearchResult::DocumentMatch {
                            id: doc.id,
                            path: doc.path.clone(),
                            content_matches: sub_results,
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
            .filter_map(|res| res)
            .collect::<Vec<SearchResult>>())
    }

    async fn build_index(&self) -> LbResult<()> {
        let account = self.get_account()?;

        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let all_ids = tree.owned_ids();
        let mut all_valid_ids = Vec::with_capacity(all_ids.len());
        let mut doc_ids = HashMap::with_capacity(all_ids.len());
        let mut cache = self.search.docs.write().await;
        cache.clear();

        for id in all_ids {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let file = tree.find(&id)?;
                let is_document = file.is_document();
                let hmac = file.document_hmac().copied();
                let has_content = hmac.is_some();

                if is_document {
                    all_valid_ids.push(id);

                    if has_content {
                        if let DocumentType::Text = DocumentType::from_file_name_using_extension(
                            &tree.name_using_links(&id, self.get_account()?)?,
                        ) {
                            doc_ids.insert(id, hmac);
                        }
                    }
                }
            }
        }

        // we could consider releasing the lock here and not hold on to it across the file io.
        // this may become needed in a future where files are fetched from the network

        for id in all_valid_ids {
            let content = if let Some(hmac) = doc_ids.get(&id) {
                let doc = self.docs.get(id, *hmac).await?;
                let doc = tree.decrypt_document(&id, &doc, account)?;

                Some(String::from_utf8_lossy(doc.as_slice()).into_owned())
            } else {
                None
            };

            cache.push(Arc::new(SearchIndexEntry {
                id,
                path: tree.id_to_path(&id, account)?,
                content,
            }));
        }

        Ok(())
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
