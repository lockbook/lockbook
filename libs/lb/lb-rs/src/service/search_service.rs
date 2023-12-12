use crate::{CoreError, CoreState, LbResult, Requester, UnexpectedError};
use crossbeam::channel::{self, Receiver, RecvTimeoutError, Sender};
use lockbook_shared::clock::get_time;
use lockbook_shared::document_repo::DocumentService;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::filename::DocumentType;
use lockbook_shared::tree_like::TreeLike;
use serde::Serialize;
use std::cmp::Ordering;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock, Mutex};
use std::thread::{self, JoinHandle, available_parallelism};
use std::time::Duration;
use sublime_fuzzy::{FuzzySearch, Scoring};
use uuid::Uuid;

const DEBOUNCE_MILLIS: u64 = 0;
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

        let join_handle = thread::spawn(move || {
            if let Err(search_err) = Self::search_loop(&results_tx, search_rx, files_info) {
                results_tx.send(SearchResult::Error(search_err));
            }
        });

        Ok(StartSearchInfo { search_tx, results_rx, join_handle })
    }

    fn recv_with_debounce(
        search_rx: &Receiver<SearchRequest>, debounce_duration: Duration,
    ) -> Result<SearchRequest, UnexpectedError> {
        let mut result = search_rx.recv()?;

        loop {
            match search_rx.recv_timeout(debounce_duration) {
                Ok(new_result) => result = new_result,
                Err(RecvTimeoutError::Timeout) => return Ok(result),
                Err(err) => return Err(UnexpectedError::from(err)),
            }
        }
    }

    fn search_loop(
        results_tx: &Sender<SearchResult>, search_rx: Receiver<SearchRequest>,
        files_info: Vec<SearchableFileInfo>, 
    ) -> Result<(), UnexpectedError> {
        let files_info = Arc::new(files_info);
        let thread_count = available_parallelism().ok().map(|thread_count| thread_count.get()).unwrap_or(2) - 1;
        let mut handles = vec![];

        let (worker_thread_tx, worker_thread_rx) = channel::unbounded::<(Arc<String>, usize)>();
        
        for _ in 0..thread_count {
            let worker_thread_rx = worker_thread_rx.clone();
            let search_result_tx = results_tx.clone();
            let searchables = files_info.clone();

            let handle: JoinHandle<Option<()>> = thread::spawn(move || {
                loop {
                    match worker_thread_rx.recv() {
                        Ok((search, searchable_index)) => {                            
                            if current_search_ts != *search_ts.lock().ok()? {
                                continue;
                            }

                            if let Some(fuzzy_match) = FuzzySearch::new(&search, &searchables[searchable_index].path)
                                .case_insensitive()
                                .score_with(&Scoring::emphasize_distance())
                                .best_match()
                            {
                                if fuzzy_match.score().is_positive() {
                                    if current_search_ts != *search_ts.lock().ok()? {
                                        continue;
                                    }

                                    search_result_tx
                                        .send(SearchResult::FileNameMatch {
                                            id: searchables[searchable_index].id,
                                            path: searchables[searchable_index].path.clone(),
                                            matched_indices: fuzzy_match.matched_indices().cloned().collect(),
                                            score: fuzzy_match.score() as i64,
                                        }).ok()?;
                                }
                            }

                            if current_search_ts != *search_ts.lock().ok()? {
                                continue;
                            }

                            if let Some(content) = &searchables[searchable_index].content {
                                let mut content_matches: Vec<ContentMatch> = Vec::new();
                
                                for paragraph in content.split("\n\n") {
                                    if current_search_ts != *search_ts.lock().ok()? {
                                        break;
                                    }

                                    if let Some(fuzzy_match) = FuzzySearch::new(&search, paragraph)
                                        .case_insensitive()
                                        .score_with(&Scoring::emphasize_distance())
                                        .best_match()
                                    {
                                        let score = fuzzy_match.score() as i64;
                
                                        if score >= LOWEST_CONTENT_SCORE_THRESHOLD {
                                            let (paragraph, matched_indices) = match Self::optimize_searched_text(
                                                paragraph,
                                                fuzzy_match.matched_indices().cloned().collect(),
                                            ) {
                                                Ok((paragraph, matched_indices)) => (paragraph, matched_indices),
                                                Err(_) => continue
                                            };
                
                                            content_matches.push(ContentMatch {
                                                paragraph,
                                                matched_indices,
                                                score,
                                            });
                                        }
                                    }
                                }


                                if current_search_ts != *search_ts.lock().ok()? {
                                    continue;
                                }
                
                                search_result_tx
                                    .send(SearchResult::FileContentMatches {
                                        id: searchables[searchable_index].id,
                                        path: searchables[searchable_index].path.clone(),
                                        content_matches,
                                    })
                                    .ok()?;
                            }
                        }
                        Err(_) => break,
                    }
                }

                None
            });

            handles.push(handle);
        }

        let mut maybe_new_search: Option<SearchRequest> = None;

        loop {
            let search = search_rx.recv().map_err(UnexpectedError::from)?;

            match search {
                SearchRequest::Search { input } => {
                    let cancel = AtomicBool::new(false);

                    let this_search = thread::spawn(|| {
                        let mut workers = vec![];

                        for i in 0..8 { // split up work equally between threads
                            let handle = thread::spawn(|| {
                                for searchable_index in 0..files_info.len() {
                                    if searchable_index % i == 0 {
                                        Self::search_unit(&input, &files_info[searchable_index]);
                                    }

                                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                        return;
                                    }
                                }
                            });

                            workers.push(handle);
                        }

                        while let Some(thread) = workers.pop() {
                            thread.join().unwrap(); // no unwraps plz
                        }

                        results_tx.send(SearchResult::SearchComplete).unwrap(); // dont unwrap
                    });


                    
                    if let Ok(new_search) = search_rx.recv() {
                        maybe_new_search = Some(new_search);

                        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                        this_search.join();
                    }
                }
                SearchRequest::EndSearch => {
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    fn search_unit(query: &str, searchable: &SearchableFileInfo) { // add cancel as parameter
        if let Some(fuzzy_match) = FuzzySearch::new(query, &searchable.path)
            .case_insensitive()
            .score_with(&Scoring::emphasize_distance())
            .best_match()
        {
            if fuzzy_match.score().is_positive() {

                // search_result_tx
                //     .send(SearchResult::FileNameMatch {
                //         id: searchables[searchable_index].id,
                //         path: searchables[searchable_index].path.clone(),
                //         matched_indices: fuzzy_match.matched_indices().cloned().collect(),
                //         score: fuzzy_match.score() as i64,
                //     }).ok()?;
            }
        }

        if let Some(content) = &searchable.content {
            let mut content_matches: Vec<ContentMatch> = Vec::new();

            for paragraph in content.split("\n\n") {
                if let Some(fuzzy_match) = FuzzySearch::new(query, paragraph)
                    .case_insensitive()
                    .score_with(&Scoring::emphasize_distance())
                    .best_match()
                {
                    let score = fuzzy_match.score() as i64;

                    if score >= LOWEST_CONTENT_SCORE_THRESHOLD {
                        let (paragraph, matched_indices) = match Self::optimize_searched_text(
                            paragraph,
                            fuzzy_match.matched_indices().cloned().collect(),
                        ) {
                            Ok((paragraph, matched_indices)) => (paragraph, matched_indices),
                            Err(_) => continue
                        };

                        content_matches.push(ContentMatch {
                            paragraph,
                            matched_indices,
                            score,
                        });
                    }
                }
            }

            // search_result_tx
            //     .send(SearchResult::FileContentMatches {
            //         id: searchables[searchable_index].id,
            //         path: searchables[searchable_index].path.clone(),
            //         content_matches,
            //     })
            //     .ok()?;
        }
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
    PathSearch
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
    EndSearch
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SearchResult {
    Error(UnexpectedError),
    FileNameMatch { 
        id: Uuid, 
        path: String, 
        matched_indices: Vec<usize>, 
        score: i64,
    },
    FileContentMatches { 
        id: Uuid, 
        path: String, 
        content_matches: Vec<ContentMatch>,
    },
    SearchComplete,
    NoMatch,
}

#[derive(Debug, Serialize)]
pub struct ContentMatch {
    pub paragraph: String,
    pub matched_indices: Vec<usize>,
    pub score: i64,
}
