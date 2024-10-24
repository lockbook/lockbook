use super::activity::RankingWeights;
use crate::logic::file_like::FileLike;
use crate::logic::filename::DocumentType;
use crate::logic::tree_like::TreeLike;
use crate::model::errors::{LbResult, UnexpectedError};
use crate::Lb;
use futures::stream::{self, FuturesUnordered, StreamExt, TryStreamExt};
use serde::Serialize;
use std::sync::Arc;
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
    pub docs: Arc<RwLock<Vec<SearchIndexEntry>>>,
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
}

impl Lb {
    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<Vec<SearchResult>> {
        if self.search.docs.read().await.is_empty() {
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

        match cfg {
            SearchConfig::Paths => self.search.search_paths(input).await,
            SearchConfig::Documents => self.search.search_content(input).await,
            SearchConfig::PathsAndDocuments => {
                let (paths, content) = tokio::join!(
                    self.search.search_paths(input),
                    self.search.search_content(input)
                );
                Ok(paths?.into_iter().chain(content?.into_iter()).collect())
            }
        }
    }

    pub async fn build_index(&self) -> LbResult<()> {
        println!("build index start");
        let start = std::time::Instant::now();

        // fetch metadata once; use single lazy tree to re-use key decryption work for all files (lb lock)
        let account = self.get_account()?;
        let (ids, keys, tree) = {
            let tx = self.ro_tx().await;
            let db = tx.db();

            let base = db.base_metadata.get().clone();
            let local = db.local_metadata.get().clone();
            let staged = base.to_staged(local);
            let mut lazy = (&staged).to_lazy();

            // populate and grab the tree's internal keycache so we don't need to share write access among workers
            let ids = lazy.owned_ids();
            for id in &ids {
                let _ = lazy.decrypt_key(id, &account);
            }

            (ids, lazy.key, Arc::new(staged))
        };

        // construct replacement index
        let search_futures = FuturesUnordered::new();
        for (i, id) in ids.into_iter().enumerate() {
            let tree = tree.clone();
            let keys = keys.clone();
            search_futures.push(async move {
                let start = std::time::Instant::now();

                let mut tree = (&*tree).to_lazy();
                tree.key = keys;

                if tree.calculate_deleted(&id).ok()? {
                    return None;
                }
                if tree.in_pending_share(&id).ok()? {
                    return None;
                }
                let name = tree.name_using_links(&id, account).ok()?;
                if DocumentType::from_file_name_using_extension(&name) != DocumentType::Text {
                    return None;
                }
                let path = tree.id_to_path(&id, account).ok()?;

                // once per file, re-lock lb to read document using up-to-date hmac (lb lock)
                // because lock has been dropped in the meantime, original `tree` is now arbitrarily out-of-date
                let tx = self.ro_tx().await;
                let db = tx.db();
                let hmac_tree = (&db.base_metadata).stage(&db.local_metadata); // not lazy bc no decryption uses this one

                let cum1 = start.elapsed();

                let Some(file) = hmac_tree.maybe_find(&id) else { return None }; // file maybe deleted since we started

                let cum2 = start.elapsed();

                let Some(&hmac) = file.document_hmac() else { return None }; // file maybe contentless

                let cum3 = start.elapsed();

                let encrypted_content = self.docs.get(id, Some(hmac)).await.ok()?; // present bc hmac from this tx

                let cum4 = start.elapsed();

                let content = if encrypted_content.value.len() <= CONTENT_MAX_LEN_BYTES {
                    // use the original tree to decrypt the content
                    let decrypted_content = tree
                        .decrypt_document(&id, &encrypted_content, account)
                        .ok()?;
                    Some(String::from_utf8_lossy(&decrypted_content).into_owned())
                } else {
                    None
                };

                println!(
                    "  indexed file {} in {:?}\t({:?}/{:?}/{:?}/{:?}",
                    i,
                    start.elapsed(),
                    cum1,
                    cum2,
                    cum3,
                    cum4
                );

                Some(SearchIndexEntry { id, path, content })
            });
        }

        // lock released while executing futures
        let replacement_index = search_futures
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        // swap in replacement index (index lock)
        *self.search.docs.write().await = replacement_index;

        println!("built index in {:?}", start.elapsed());

        Ok(())
    }

    pub fn spawn_build_index(&self) {
        tokio::spawn({
            let lb = self.clone();
            async move { lb.build_index().await }
        });
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
        // read lock held while constructing futures (data cloned into futures)
        let search_futures = FuturesUnordered::new();
        let docs = self.docs.read().await;
        for doc in docs.iter() {
            let id = doc.id;
            let path = doc.path.clone();
            let content = doc.content.clone();
            search_futures.push(async move {
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
                        return Some(SearchResult::DocumentMatch { id, path, content_matches });
                    }
                }
                None
            });
        }

        // lock released while executing futures
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
