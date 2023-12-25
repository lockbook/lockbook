use crate::{CoreError, CoreState, LbResult, Requester, UnexpectedError};
use crossbeam::channel::{self, Receiver, Sender};
use lockbook_shared::document_repo::DocumentService;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::filename::DocumentType;
use lockbook_shared::tree_like::TreeLike;
use serde::Serialize;
use std::cmp::Ordering;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::{self, available_parallelism, JoinHandle};
use sublime_fuzzy::{FuzzySearch, Scoring};
use uuid::Uuid;

const LOWEST_CONTENT_SCORE_THRESHOLD: i64 = 170;

const MAX_CONTENT_MATCH_LENGTH: usize = 400;
const IDEAL_CONTENT_MATCH_LENGTH: usize = 150;
const CONTENT_MATCH_PADDING: usize = 8;

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

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    pub(crate) fn search_file_paths(&mut self, input: &str) -> LbResult<Vec<SearchResultItem>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();

        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;
        let mut results = Vec::new();

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let file = tree.find(&id)?;

                if file.is_document() {
                    let path = tree.id_to_path(&id, account)?;

                    if let Some(m) = FuzzySearch::new(input, &path)
                        .case_insensitive()
                        .best_match()
                    {
                        results.push(SearchResultItem {
                            id,
                            path,
                            score: m.score(),
                            matched_indices: m.matched_indices().cloned().collect(),
                        });
                    }
                }
            }
        }

        results.sort();

        Ok(results)
    }

    pub(crate) fn start_search(&mut self, search_type: SearchType) -> LbResult<StartSearchInfo> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();

        let account = self.db.account.get().ok_or(CoreError::AccountNonexistent)?;
        let mut files_info = Vec::new();

        for id in tree.owned_ids() {
            if !tree.calculate_deleted(&id)? && !tree.in_pending_share(&id)? {
                let file = tree.find(&id)?;

                if file.is_document() {
                    let content = match search_type {
                        SearchType::PathAndContentSearch => {
                            match DocumentType::from_file_name_using_extension(
                                &tree.name_using_links(&id, account)?,
                            ) {
                                DocumentType::Text => {
                                    let doc = tree.read_document(&self.docs, &id, account)?;
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
                    })
                }
            }
        }

        let (search_tx, search_rx) = channel::unbounded::<SearchRequest>();
        let (results_tx, results_rx) = channel::unbounded::<SearchResult>();

        println!("collected all docs");

        let join_handle = thread::spawn(move || {
            if let Err(search_err) = Self::search_loop(&results_tx, search_rx, files_info) {
                let _ = results_tx.send(SearchResult::Error(search_err));
            }
        });

        Ok(StartSearchInfo { search_tx, results_rx, join_handle })
    }

    fn search_loop(
        results_tx: &Sender<SearchResult>, search_rx: Receiver<SearchRequest>,
        files_info: Vec<SearchableFileInfo>,
    ) -> Result<(), UnexpectedError> {
        println!("search thread launched");

        let files_info = Arc::new(files_info);
        let thread_count = available_parallelism()
            .ok()
            .map(|thread_count| thread_count.get())
            .unwrap_or(2)
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
                None => search_rx.recv().map_err(UnexpectedError::from)?,
            };

            match search {
                SearchRequest::Search { input } => {
                    println!("search recieved");

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

                    // eliminate no search result and just include

                    let this_search = thread::spawn(move || {
                        let mut workers = vec![];

                        let cancel = Arc::new(AtomicBool::new(false));
                        let offset = files_info.len() / thread_count;
                        let search_result_tx = search_result_tx.clone();
                        let input = input.clone();
                        let files_info = files_info.clone();

                        for i in 0..thread_count {
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
            if fuzzy_match.score().is_positive() {
                search_result_tx
                    .send(SearchResult::FileNameMatch {
                        id: searchable.id,
                        path: searchable.path.clone(),
                        matched_indices: fuzzy_match.matched_indices().cloned().collect(),
                        score: fuzzy_match.score() as i64,
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
                    let score = fuzzy_match.score() as i64;

                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        return Ok(());
                    }

                    if score >= LOWEST_CONTENT_SCORE_THRESHOLD {
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

            search_result_tx
                .send(SearchResult::FileContentMatches {
                    id: searchable.id,
                    path: searchable.path.clone(),
                    content_matches,
                })
                .map_err(UnexpectedError::from)?;
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
    pub join_handle: JoinHandle<()>,
}

struct SearchableFileInfo {
    id: Uuid,
    path: String,
    content: Option<String>,
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
