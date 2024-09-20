use crate::logic::crypto::{DecryptedDocument, EncryptedDocument};
use crate::logic::file_like::FileLike;
use crate::logic::filename::DocumentType;
use crate::logic::tree_like::TreeLike;
use crate::model::errors::LbResult;
use crate::model::file::File;
use crate::{Lb, UnexpectedError};
use crossbeam::channel::{self, Receiver, Sender};
use futures::stream::FuturesUnordered;
use futures::{future, StreamExt};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::{self, available_parallelism};
use sublime_fuzzy::{FuzzySearch, Scoring};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::activity::{RankingWeights, Stats};

const CONTENT_SCORE_THRESHOLD: i64 = 170;
const PATH_SCORE_THRESHOLD: i64 = 10;

const MAX_CONTENT_MATCH_LENGTH: usize = 400;
const IDEAL_CONTENT_MATCH_LENGTH: usize = 150;
const CONTENT_MATCH_PADDING: usize = 8;

const ACTIVITY_WEIGHT: f32 = 0.2;
const FUZZY_WEIGHT: f32 = 0.8;

#[derive(Clone, Default)]
pub struct SearchIndex {
    pub docs: Arc<Mutex<Vec<SearchIndexEntry>>>,
}

pub struct SearchIndexEntry {
    pub file: File,
    pub path: String,
    pub content: Option<DecryptedDocument>,
}

#[derive(Copy, Clone)]
pub enum SearchConfig {
    Paths,
    Documents,
    PathsAndDocuments,
}

impl Lb {
    pub async fn search(&self, input: &str, cfg: SearchConfig) -> LbResult<()> {
        todo!()
    }

    async fn build_index(&self) -> LbResult<()> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let all_ids = tree.owned_ids();
        let mut doc_ids = Vec::with_capacity(all_ids.len());
        let mut content = HashMap::new();

        for id in all_ids {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let file = tree.find(&id)?;
                let is_document = file.is_document();
                let hmac = file.document_hmac().copied();
                let has_content = hmac.is_some();

                if is_document && has_content {
                    match DocumentType::from_file_name_using_extension(
                        &tree.name_using_links(&id, self.get_account()?)?,
                    ) {
                        DocumentType::Text => doc_ids.push((id, hmac)),
                        _ => {}
                    }
                }
            }
        }
        
        // we could consider releasing the lock here and not hold on to it across the file io. 
        // this may become needed in a future where files are fetched from the network
    
        let mut content_futures = FuturesUnordered::new();

        for (id, hmac) in doc_ids {
            content_futures.push(async move { (id, self.docs.get(id, hmac).await) });
        }

        while let Some((id, res)) = content_futures.next().await {
            let res = res?;
            tree.decrypt_document(&id, &res, self.get_account()?);
            content.insert(id, res);
        }

        Ok(())
    }
}

#[derive(Serialize, Debug, Eq, PartialEq)]
pub struct SearchResultItem {
    pub id: Uuid,
    pub path: String,
    pub score: isize,
    pub matched_indices: Vec<usize>,
}

impl Ord for SearchResultItem {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.score.cmp(&other.score) {
            Ordering::Greater => Ordering::Less,
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => self.path.cmp(&other.path),
        }
    }
}

impl PartialOrd for SearchResultItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Lb {
    pub async fn search_file_paths(&self, input: &str) -> LbResult<Vec<SearchResultItem>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut activity_metrics = db.doc_events.get().iter().get_activity_metrics();
        let activity_weights = RankingWeights::default();
        self.normalize(&mut activity_metrics);

        let file_scores: HashMap<Uuid, i64> = activity_metrics
            .into_iter()
            .map(|metric| (metric.id, metric.score(activity_weights)))
            .collect();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let account = self.get_account()?;
        let mut results = Vec::new();

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let file = tree.find(&id)?;

                if file.is_document() {
                    let path = tree.id_to_path(&id, account)?;

                    if let Some(fuzzy_match) = FuzzySearch::new(input, &path)
                        .case_insensitive()
                        .best_match()
                    {
                        let score = ((fuzzy_match.score().min(600) as f32 * FUZZY_WEIGHT)
                            + (file_scores.get(&id).cloned().unwrap_or_default() as f32
                                * ACTIVITY_WEIGHT)) as i64;

                        if score > PATH_SCORE_THRESHOLD {
                            results.push(SearchResultItem {
                                id,
                                path,
                                score: score as isize,
                                matched_indices: fuzzy_match.matched_indices().cloned().collect(),
                            });
                        }
                    }
                }
            }
        }

        results.sort();

        Ok(results)
    }

    pub fn start_search(&self, search_type: SearchType) -> StartSearchInfo {
        let (search_tx, search_rx) = channel::unbounded::<SearchRequest>();
        let (results_tx, results_rx) = channel::unbounded::<SearchResult>();

        let core = self.clone();
        let results_tx_c = results_tx.clone();

        tokio::spawn(async move {
            if let Err(err) = core
                .start_search_inner(search_type, results_tx, search_rx)
                .await
            {
                let _ = results_tx_c.send(SearchResult::Error(err.into()));
            }
        });

        StartSearchInfo { search_tx, results_rx }
    }

    pub(crate) async fn start_search_inner(
        &self, search_type: SearchType, results_tx: Sender<SearchResult>,
        search_rx: Receiver<SearchRequest>,
    ) -> LbResult<()> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut activity_metrics = db.doc_events.get().iter().get_activity_metrics();
        let activity_weights = RankingWeights::default();
        self.normalize(&mut activity_metrics);

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let account = self.get_account()?;
        let mut files_info = Vec::new();

        let file_scores: HashMap<Uuid, i64> = activity_metrics
            .into_iter()
            .map(|metric| (metric.id, metric.score(activity_weights).min(600)))
            .collect();

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let file = tree.find(&id)?;
                let id = *file.id();

                if file.is_document() {
                    let content = match search_type {
                        SearchType::PathAndContentSearch => {
                            match DocumentType::from_file_name_using_extension(
                                &tree.name_using_links(&id, account)?,
                            ) {
                                DocumentType::Text => {
                                    let doc = self.read_document_helper(id, &mut tree).await?;
                                    match String::from_utf8(doc) {
                                        Ok(str) => Some(str),
                                        Err(utf_8) => {
                                            error!("failed to read {id}, {utf_8}");
                                            None
                                        }
                                    }
                                }
                                _ => None,
                            }
                        }
                        SearchType::PathSearch => None,
                    };

                    files_info.push(SearchableFileInfo {
                        id,
                        path: tree.id_to_path(&id, account)?,
                        content,
                        activity_score: file_scores.get(&id).cloned().unwrap_or_default(),
                    })
                }
            }
        }

        thread::spawn(move || {
            if let Err(err) = Self::search_loop(&results_tx, search_rx, files_info) {
                let _ = results_tx.send(SearchResult::Error(err));
            }
        });

        Ok(())
    }

    fn search_loop(
        results_tx: &Sender<SearchResult>, search_rx: Receiver<SearchRequest>,
        files_info: Vec<SearchableFileInfo>,
    ) -> Result<(), UnexpectedError> {
        let files_info = Arc::new(files_info);
        let thread_count = available_parallelism()
            .ok()
            .map(|thread_count| thread_count.get())
            .unwrap_or(2)
            .max(2)
            - 1;

        let mut maybe_new_search = None;

        loop {
            while let Ok(request) = search_rx.try_recv() {
                maybe_new_search = Some(request);
            }

            let search = match maybe_new_search {
                Some(new_seach) => {
                    maybe_new_search = None;
                    new_seach
                }
                None => match search_rx.recv() {
                    Ok(new_search) => new_search,
                    Err(_) => return Ok(()),
                },
            };

            match search {
                SearchRequest::Search { input } => {
                    results_tx
                        .send(SearchResult::StartOfSearch)
                        .map_err(UnexpectedError::from)?;

                    if input.is_empty() {
                        results_tx
                            .send(SearchResult::EndOfSearch)
                            .map_err(UnexpectedError::from)?;

                        continue;
                    }

                    let cancel = Arc::new(AtomicBool::new(false));
                    let files_info = files_info.clone();
                    let search_result_tx = results_tx.clone();

                    let this_search = thread::spawn(move || {
                        let mut workers = vec![];

                        let cancel = Arc::new(AtomicBool::new(false));
                        let offset = (files_info.len() / thread_count).max(1);
                        let search_result_tx = search_result_tx.clone();
                        let input = input.clone();
                        let files_info = files_info.clone();

                        let threads_used = thread_count.min(files_info.len());

                        for i in 0..threads_used {
                            // split up work equally between threads

                            let cancel = cancel.clone();
                            let input = input.clone();
                            let files_info = files_info.clone();
                            let search_result_tx = search_result_tx.clone();

                            let handle = thread::spawn(move || {
                                let start = offset * i;
                                let end = offset * (i + 1);

                                for searchable_index in start..end {
                                    if let Err(err) = Self::search_unit(
                                        &input,
                                        &files_info[searchable_index],
                                        &search_result_tx,
                                        &cancel,
                                    ) {
                                        let _ = search_result_tx.send(SearchResult::Error(err));
                                    }

                                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                        return;
                                    }
                                }
                            });

                            workers.push(handle);
                        }

                        while let Some(thread) = workers.pop() {
                            if thread.join().is_err() {
                                let _ = search_result_tx.send(SearchResult::Error(
                                    UnexpectedError::new("cannot join search worker thread"),
                                ));
                            }
                        }

                        let _ = search_result_tx.send(SearchResult::EndOfSearch);
                    });

                    if let Ok(new_search) = search_rx.recv() {
                        maybe_new_search = Some(new_search);

                        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                        this_search.join().map_err(|_| {
                            UnexpectedError::new("cannot join managing search thread")
                        })?;
                    }
                }
                SearchRequest::EndSearch => {
                    return Ok(());
                }
            }
        }
    }

    fn search_unit(
        query: &str, searchable: &SearchableFileInfo, search_result_tx: &Sender<SearchResult>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<(), UnexpectedError> {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        if let Some(fuzzy_match) = FuzzySearch::new(query, &searchable.path)
            .case_insensitive()
            .score_with(&Scoring::emphasize_distance())
            .best_match()
        {
            let score = ((fuzzy_match.score().min(600) as f32 * FUZZY_WEIGHT)
                + (searchable.activity_score as f32 * ACTIVITY_WEIGHT))
                as i64;

            if score > PATH_SCORE_THRESHOLD {
                search_result_tx
                    .send(SearchResult::FileNameMatch {
                        id: searchable.id,
                        path: searchable.path.clone(),
                        matched_indices: fuzzy_match.matched_indices().cloned().collect(),
                        score,
                    })
                    .map_err(UnexpectedError::from)?;
            }
        }

        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        if let Some(content) = &searchable.content {
            let mut content_matches: Vec<ContentMatch> = Vec::new();

            for paragraph in content.split("\n\n") {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(());
                }

                if let Some(fuzzy_match) = FuzzySearch::new(query, paragraph)
                    .case_insensitive()
                    .score_with(&Scoring::emphasize_distance())
                    .best_match()
                {
                    let score = ((fuzzy_match.score().min(600) as f32 * FUZZY_WEIGHT)
                        + (searchable.activity_score as f32 * ACTIVITY_WEIGHT))
                        as i64;

                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        return Ok(());
                    }

                    if score > CONTENT_SCORE_THRESHOLD {
                        let (paragraph, matched_indices) = match Self::optimize_searched_text(
                            paragraph,
                            fuzzy_match.matched_indices().cloned().collect(),
                        ) {
                            Ok((paragraph, matched_indices)) => (paragraph, matched_indices),
                            Err(_) => continue,
                        };

                        content_matches.push(ContentMatch { paragraph, matched_indices, score });
                    }
                }
            }

            if !content_matches.is_empty() {
                search_result_tx
                    .send(SearchResult::FileContentMatches {
                        id: searchable.id,
                        path: searchable.path.clone(),
                        content_matches,
                    })
                    .map_err(UnexpectedError::from)?;
            }
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

impl SearchResult {
    pub fn get_score(&self) -> Option<i64> {
        match self {
            SearchResult::FileNameMatch { id: _, path: _, matched_indices: _, score } => {
                Some(*score)
            }
            SearchResult::FileContentMatches { id: _, path: _, content_matches } => {
                Some(content_matches[0].score)
            }
            _ => None,
        }
    }
}

pub enum SearchType {
    PathAndContentSearch,
    PathSearch,
}

pub struct StartSearchInfo {
    pub search_tx: Sender<SearchRequest>,
    pub results_rx: Receiver<SearchResult>,
}

struct SearchableFileInfo {
    id: Uuid,
    path: String,
    content: Option<String>,
    activity_score: i64,
}

#[derive(Clone)]
pub enum SearchRequest {
    Search { input: String },
    EndSearch,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SearchResult {
    Error(UnexpectedError),
    StartOfSearch,
    FileNameMatch { id: Uuid, path: String, matched_indices: Vec<usize>, score: i64 },
    FileContentMatches { id: Uuid, path: String, content_matches: Vec<ContentMatch> },
    EndOfSearch,
}

#[derive(Debug, Serialize)]
pub struct ContentMatch {
    pub paragraph: String,
    pub matched_indices: Vec<usize>,
    pub score: i64,
}
