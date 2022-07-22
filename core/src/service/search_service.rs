use crate::model::filename::DocumentType;
use crate::model::repo::RepoSource;
use crate::{CoreError, RequestContext, UnexpectedError};
use crossbeam::channel;
use crossbeam::channel::{Receiver, RecvTimeoutError, Sender};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use lockbook_models::file_metadata::DecryptedFiles;
use lockbook_models::tree::FileMetadata;
use serde::Serialize;
use std::cmp::Ordering;
use std::sync::atomic::AtomicBool;
use std::sync::{atomic, Arc};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use sublime_fuzzy::{FuzzySearch, Scoring};
use uuid::Uuid;

const DEBOUNCE_MILLIS: u64 = 500;
const LOWEST_CONTENT_SCORE_THRESHOLD: i64 = 170;

const MAX_CONTENT_MATCH_LENGTH: usize = 400;
const IDEAL_CONTENT_MATCH_LENGTH: usize = 150;
const CONTENT_MATCH_PADDING: usize = 8;

#[derive(Debug, Eq, PartialEq)]
pub struct SearchResultItem {
    pub id: Uuid,
    pub path: String,
    pub score: i64,
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

impl RequestContext<'_, '_> {
    pub fn search_file_paths(&mut self, input: &str) -> Result<Vec<SearchResultItem>, CoreError> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let matcher = SkimMatcherV2::default();

        let mut results = Vec::new();
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        for &id in files.keys() {
            let path = Self::path_by_id_helper(&files, id)?;

            if let Some(score) = matcher.fuzzy_match(&path, input) {
                results.push(SearchResultItem { id, path, score });
            }
        }
        results.sort();

        Ok(results)
    }

    pub fn start_search(&mut self) -> Result<StartSearchInfo, CoreError> {
        let files_not_deleted: DecryptedFiles =
            self.get_all_not_deleted_metadata(RepoSource::Local)?;

        let mut files_info = Vec::new();

        for file in files_not_deleted.values() {
            if file.is_document() {
                let content =
                    match DocumentType::from_file_name_using_extension(&file.decrypted_name) {
                        DocumentType::Text => self
                            .read_document(self.config, RepoSource::Local, file.id)
                            .map(|content| String::from(String::from_utf8_lossy(content.as_ref())))
                            .map(Some)?,
                        _ => None,
                    };

                files_info.push(SearchableFileInfo {
                    id: file.id,
                    path: RequestContext::path_by_id_helper(&files_not_deleted, file.id)?,
                    content,
                })
            }
        }

        let (search_tx, search_rx) = channel::unbounded::<SearchRequest>();
        let (results_tx, results_rx) = channel::unbounded::<SearchResult>();
        let join_handle = thread::spawn(move || {
            if let Err(search_err) = Self::search_loop(&results_tx, search_rx, files_info) {
                if let Err(err) = results_tx.send(SearchResult::Error(search_err)) {
                    warn!("Send failed: {:#?}", err);
                }
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
        let mut should_continue = Arc::new(AtomicBool::new(true));
        let debounce_duration = Duration::from_millis(DEBOUNCE_MILLIS);

        loop {
            let search = RequestContext::recv_with_debounce(&search_rx, debounce_duration)?;

            match search {
                SearchRequest::Search { input } => {
                    should_continue.store(false, atomic::Ordering::Relaxed);
                    should_continue = Arc::new(AtomicBool::new(true));

                    let results_tx = results_tx.clone();
                    let files_info = files_info.clone();
                    let should_continue = should_continue.clone();
                    let input = input.clone();

                    thread::spawn(move || {
                        if let Err(search_err) =
                            RequestContext::search(&results_tx, files_info, should_continue, input)
                        {
                            if let Err(err) = results_tx.send(SearchResult::Error(search_err)) {
                                warn!("Send failed: {:#?}", err);
                            }
                        }
                    });
                }
                SearchRequest::EndSearch => {
                    should_continue.store(false, atomic::Ordering::Relaxed);
                    return Ok(());
                }
                SearchRequest::StopCurrentSearch => {
                    should_continue.store(false, atomic::Ordering::Relaxed)
                }
            }
        }
    }

    fn search(
        results_tx: &Sender<SearchResult>, files_info: Arc<Vec<SearchableFileInfo>>,
        should_continue: Arc<AtomicBool>, search: String,
    ) -> Result<(), UnexpectedError> {
        let mut no_matches = true;

        RequestContext::search_file_names(
            results_tx,
            &should_continue,
            &files_info,
            &search,
            &mut no_matches,
        )?;

        if should_continue.load(atomic::Ordering::Relaxed) {
            RequestContext::search_file_contents(
                results_tx,
                &should_continue,
                &files_info,
                &search,
                &mut no_matches,
            )?;

            if no_matches && should_continue.load(atomic::Ordering::Relaxed) {
                results_tx.send(SearchResult::NoMatch)?;
            }
        }

        Ok(())
    }

    fn search_file_names(
        results_tx: &Sender<SearchResult>, should_continue: &Arc<AtomicBool>,
        files_info: &Arc<Vec<SearchableFileInfo>>, search: &str, no_matches: &mut bool,
    ) -> Result<(), UnexpectedError> {
        for info in files_info.as_ref() {
            if !should_continue.load(atomic::Ordering::Relaxed) {
                return Ok(());
            }

            if let Some(fuzzy_match) = FuzzySearch::new(search, &info.path)
                .case_insensitive()
                .score_with(&Scoring::emphasize_distance())
                .best_match()
            {
                if fuzzy_match.score().is_positive() {
                    if *no_matches {
                        *no_matches = false;
                    }

                    if !should_continue.load(atomic::Ordering::Relaxed) {
                        return Ok(());
                    }

                    if results_tx
                        .send(SearchResult::FileNameMatch {
                            id: info.id,
                            path: info.path.clone(),
                            matched_indices: fuzzy_match.matched_indices().cloned().collect(),
                            score: fuzzy_match.score() as i64,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn search_file_contents(
        results_tx: &Sender<SearchResult>, should_continue: &Arc<AtomicBool>,
        files_info: &Arc<Vec<SearchableFileInfo>>, search: &str, no_matches: &mut bool,
    ) -> Result<(), UnexpectedError> {
        for info in files_info.as_ref() {
            if !should_continue.load(atomic::Ordering::Relaxed) {
                return Ok(());
            }

            if let Some(content) = &info.content {
                let mut content_matches: Vec<ContentMatch> = Vec::new();

                for paragraph in content.split("\n\n") {
                    if let Some(fuzzy_match) = FuzzySearch::new(search, paragraph)
                        .case_insensitive()
                        .score_with(&Scoring::emphasize_distance())
                        .best_match()
                    {
                        let score = fuzzy_match.score() as i64;

                        if score >= LOWEST_CONTENT_SCORE_THRESHOLD {
                            let (paragraph, matched_indices) =
                                RequestContext::optimize_searched_text(
                                    paragraph,
                                    fuzzy_match.matched_indices().cloned().collect(),
                                )?;

                            content_matches.push(ContentMatch {
                                paragraph,
                                matched_indices,
                                score,
                            });
                        }
                    }
                }

                if !content_matches.is_empty() {
                    if *no_matches {
                        *no_matches = false;
                    }

                    if !should_continue.load(atomic::Ordering::Relaxed) {
                        return Ok(());
                    }

                    if results_tx
                        .send(SearchResult::FileContentMatches {
                            id: info.id,
                            path: info.path.clone(),
                            content_matches,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
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
            UnexpectedError("No matched indices.".to_string())
        })?;

        let last_match = new_indices.last().ok_or_else(|| {
            warn!("A fuzzy match happened but there are no matched indices.");
            UnexpectedError("No matched indices.".to_string())
        })?;

        if *last_match < IDEAL_CONTENT_MATCH_LENGTH {
            new_paragraph = new_paragraph
                .chars()
                .take(IDEAL_CONTENT_MATCH_LENGTH + CONTENT_MATCH_PADDING)
                .collect();

            new_paragraph.push_str("...");
        } else {
            if *first_match > CONTENT_MATCH_PADDING as usize {
                let at_least_take = new_paragraph.len() - first_match + CONTENT_MATCH_PADDING;

                let deleted_chars_len = if at_least_take > IDEAL_CONTENT_MATCH_LENGTH {
                    first_match - CONTENT_MATCH_PADDING
                } else {
                    new_paragraph.len() - IDEAL_CONTENT_MATCH_LENGTH
                };

                index_offset = deleted_chars_len - 3;

                new_paragraph = new_paragraph.chars().skip(deleted_chars_len).collect();
                new_paragraph.insert_str(0, "...");
            }

            if new_paragraph.len() > IDEAL_CONTENT_MATCH_LENGTH + CONTENT_MATCH_PADDING + 3 {
                let at_least_take = *last_match - index_offset + CONTENT_MATCH_PADDING;

                let take_chars_len = if at_least_take > IDEAL_CONTENT_MATCH_LENGTH {
                    at_least_take
                } else {
                    IDEAL_CONTENT_MATCH_LENGTH - (last_match - index_offset)
                };

                new_paragraph = new_paragraph.chars().take(take_chars_len).collect();
                new_paragraph.push_str("...");
            }

            if new_paragraph.len() > MAX_CONTENT_MATCH_LENGTH {
                new_paragraph = new_paragraph
                    .chars()
                    .take(MAX_CONTENT_MATCH_LENGTH)
                    .collect();

                new_indices = new_indices
                    .into_iter()
                    .filter(|index| (*index - index_offset) < MAX_CONTENT_MATCH_LENGTH)
                    .collect()
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
    StopCurrentSearch,
}

#[derive(Debug)]
pub enum SearchResult {
    Error(UnexpectedError),
    FileNameMatch { id: Uuid, path: String, matched_indices: Vec<usize>, score: i64 },
    FileContentMatches { id: Uuid, path: String, content_matches: Vec<ContentMatch> },
    NoMatch,
}

#[derive(Serialize, Debug)]
pub struct ContentMatch {
    pub paragraph: String,
    pub matched_indices: Vec<usize>,
    pub score: i64,
}
