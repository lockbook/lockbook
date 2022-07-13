use crate::model::repo::RepoSource;
use crate::{CoreError, RequestContext, UnexpectedError};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use itertools::Itertools;
use lockbook_models::file_metadata::{DecryptedFiles, FileType};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use sublime_fuzzy::{FuzzySearch, Scoring};
use uuid::Uuid;

const DEBOUNCE_MILLIS: u64 = 500;
const LOWEST_SCORE_THRESHOLD: i64 = 100;

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

    pub fn start_search(
        &mut self, results_tx: Sender<SearchResult>, search_rx: Receiver<SearchRequest>,
    ) -> Result<JoinHandle<()>, CoreError> {
        let files_not_deleted: DecryptedFiles =
            self.get_all_not_deleted_metadata(RepoSource::Local)?;

        let mut ids: Vec<Uuid> = Vec::new();
        let mut paths: HashMap<Uuid, String> = HashMap::new();
        let mut files_contents: HashMap<Uuid, String> = HashMap::new();

        for (id, file) in &files_not_deleted {
            if file.file_type == FileType::Document {
                ids.push(*id);
                paths.insert(
                    file.id,
                    RequestContext::path_by_id_helper(&files_not_deleted, file.id)?,
                );

                if file.decrypted_name.as_str().ends_with(".txt")
                    || file.decrypted_name.as_str().ends_with(".md")
                {
                    files_contents.insert(
                        file.id,
                        String::from(String::from_utf8_lossy(&self.read_document(
                            self.config,
                            RepoSource::Local,
                            file.id,
                        )?)),
                    );
                }
            }
        }

        Ok(thread::spawn(move || {
            if let Err(e) = Self::search(
                &results_tx,
                search_rx,
                Arc::new(ids),
                Arc::new(paths),
                Arc::new(files_contents),
            ) {
                if let Err(_) =
                    results_tx.send(SearchResult::Error(UnexpectedError(format!("{:?}", e))))
                {
                    // can't send the error, so nothing to do
                }
            }
        }))
    }

    fn search(
        results_tx: &Sender<SearchResult>, search_rx: Receiver<SearchRequest>, ids: Arc<Vec<Uuid>>,
        paths: Arc<HashMap<Uuid, String>>, files_contents: Arc<HashMap<Uuid, String>>,
    ) -> Result<(), UnexpectedError> {
        let mut last_search = match search_rx.recv() {
            Ok(last_search) => last_search,
            Err(_) => return Ok(()),
        };

        let mut should_continue = Arc::new(Mutex::new(true));

        match &last_search {
            SearchRequest::Search { input } => {
                RequestContext::spawn_search(
                    results_tx.clone(),
                    ids.clone(),
                    paths.clone(),
                    files_contents.clone(),
                    should_continue.clone(),
                    input.clone(),
                );
            }
            SearchRequest::EndSearch => return Ok(()),
            SearchRequest::StopCurrentSearch => {}
        };

        let mut skip_channel_check = false;

        loop {
            if !skip_channel_check {
                last_search = match search_rx.recv() {
                    Ok(last_search) => last_search,
                    Err(_) => return Ok(()),
                };

                *should_continue.lock()? = false;
            } else {
                skip_channel_check = false;
            }

            thread::sleep(Duration::from_millis(DEBOUNCE_MILLIS));

            if let Some(search) = search_rx.try_iter().last() {
                last_search = search;
                skip_channel_check = true;
                continue;
            }

            match &last_search {
                SearchRequest::Search { input } => {
                    should_continue = Arc::new(Mutex::new(true));

                    RequestContext::spawn_search(
                        results_tx.clone(),
                        ids.clone(),
                        paths.clone(),
                        files_contents.clone(),
                        should_continue.clone(),
                        input.clone(),
                    );
                }
                SearchRequest::EndSearch => return Ok(()),
                SearchRequest::StopCurrentSearch => {}
            };
        }
    }

    fn spawn_search(
        results_tx: Sender<SearchResult>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>,
        files_contents: Arc<HashMap<Uuid, String>>, should_continue: Arc<Mutex<bool>>,
        input: String,
    ) {
        thread::spawn(move || {
            if let Err(e) = RequestContext::search_loop(
                results_tx.clone(),
                ids,
                paths,
                files_contents,
                should_continue,
                input,
            ) {
                if let Err(_) = results_tx.send(SearchResult::Error(e)) {
                    // can't send the error, so nothing to do
                }
            }
        });
    }

    fn search_loop(
        results_tx: Sender<SearchResult>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>,
        files_contents: Arc<HashMap<Uuid, String>>, should_continue: Arc<Mutex<bool>>,
        input: String,
    ) -> Result<(), UnexpectedError> {
        let mut no_matches = true;

        RequestContext::search_file_names(
            &results_tx,
            &should_continue,
            &ids,
            &paths,
            &input,
            &mut no_matches,
        )?;
        if *should_continue.lock()? {
            RequestContext::search_file_contents(
                &results_tx,
                &should_continue,
                &ids,
                &paths,
                &files_contents,
                &input,
                &mut no_matches,
            )?;
        }

        if no_matches && *should_continue.lock()? {
            if let Err(_) = results_tx.send(SearchResult::NoMatch) {
                // session already ended so no point
            }
        }

        Ok(())
    }

    fn search_file_names(
        results_tx: &Sender<SearchResult>, should_continue: &Arc<Mutex<bool>>,
        ids: &Arc<Vec<Uuid>>, paths: &Arc<HashMap<Uuid, String>>, search: &str,
        no_matches: &mut bool,
    ) -> Result<(), UnexpectedError> {
        for id in ids.as_ref() {
            if !*should_continue.lock()? {
                return Ok(());
            }

            if let Some(fuzzy_match) = FuzzySearch::new(search, &paths[id])
                .case_insensitive()
                .score_with(&Scoring::emphasize_distance())
                .best_match()
            {
                if fuzzy_match.score().is_positive() {
                    if *no_matches {
                        *no_matches = false;
                    }

                    if !*should_continue.lock()? {
                        return Ok(());
                    }

                    if let Err(_) = results_tx.send(SearchResult::FileNameMatch {
                        id: id.clone(),
                        path: paths[id].clone(),
                        matched_indices: fuzzy_match.matched_indices().cloned().collect(),
                        score: fuzzy_match.score() as i64,
                    }) {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn search_file_contents(
        results_tx: &Sender<SearchResult>, should_continue: &Arc<Mutex<bool>>,
        ids: &Arc<Vec<Uuid>>, paths: &Arc<HashMap<Uuid, String>>,
        files_contents: &Arc<HashMap<Uuid, String>>, search: &str, no_matches: &mut bool,
    ) -> Result<(), UnexpectedError> {
        for id in ids.as_ref() {
            if !*should_continue.lock()? {
                return Ok(());
            }

            let paragraphs = match files_contents.get(id) {
                None => continue,
                Some(content) => content.split("\n\n"),
            };

            let mut content_matches: Vec<ContentMatch> = Vec::new();

            for paragraph in paragraphs {
                // matcher.fuzzy_indices(paragraph, search)
                if let Some(fuzzy_match) = FuzzySearch::new(search, paragraph)
                    .case_insensitive()
                    .score_with(&Scoring::emphasize_distance())
                    .best_match()
                {
                    if fuzzy_match.score() >= LOWEST_SCORE_THRESHOLD as isize {
                        let (paragraph, matched_indices) = RequestContext::optimize_searched_text(
                            paragraph,
                            fuzzy_match.matched_indices().cloned().collect(),
                        );

                        content_matches.push(ContentMatch {
                            paragraph,
                            matched_indices,
                            score: fuzzy_match.score() as i64,
                        });
                    }
                }
            }

            if !content_matches.is_empty() {
                if *no_matches {
                    *no_matches = false;
                }

                if !*should_continue.lock()? {
                    return Ok(());
                }

                if let Err(_) = results_tx.send(SearchResult::FileContentMatches {
                    id: id.clone(),
                    path: paths[id].clone(),
                    content_matches,
                }) {
                    break;
                }
            }
        }

        Ok(())
    }

    fn optimize_searched_text(
        paragraph: &str, matched_indices: Vec<usize>,
    ) -> (String, Vec<usize>) {
        // let mut distance_between_indcies: Vec<usize> = Vec::new();

        if paragraph.len() <= IDEAL_PARAGRAPH_LENGTH {
            return (paragraph.to_string(), matched_indices);
        }

        let mut index_offset: usize = 0;
        let mut new_paragraph: String = paragraph.to_string();

        match (matched_indices.first(), matched_indices.last()) {
            (Some(first_match), Some(last_match)) => {
                if *last_match < IDEAL_PARAGRAPH_LENGTH {
                    new_paragraph = new_paragraph
                        .chars()
                        .take(IDEAL_PARAGRAPH_LENGTH + PARAGRAPH_SLICE_PADDING)
                        .collect();
                    new_paragraph.push_str("...");

                    return (
                        new_paragraph,
                        matched_indices
                            .iter()
                            .map(|index| *index - index_offset)
                            .collect(),
                    );
                }

                if *first_match > PARAGRAPH_SLICE_PADDING as usize {
                    let at_least_take = new_paragraph.len() - first_match + PARAGRAPH_SLICE_PADDING;

                    let deleted_chars_len = if at_least_take > IDEAL_PARAGRAPH_LENGTH {
                        first_match - PARAGRAPH_SLICE_PADDING
                    } else {
                        new_paragraph.len() - IDEAL_PARAGRAPH_LENGTH
                    };

                    index_offset = deleted_chars_len - 3;

                    new_paragraph = new_paragraph.chars().skip(deleted_chars_len).collect();
                    new_paragraph.insert_str(0, "...");

                    if new_paragraph.len() <= IDEAL_PARAGRAPH_LENGTH {
                        return (
                            new_paragraph,
                            matched_indices
                                .iter()
                                .map(|index| *index - index_offset)
                                .collect(),
                        );
                    }
                }

                if matched_indices.len() - last_match > PARAGRAPH_SLICE_PADDING {
                    let at_least_take = *last_match - index_offset + PARAGRAPH_SLICE_PADDING;

                    let take_chars_len = if at_least_take > IDEAL_PARAGRAPH_LENGTH {
                        at_least_take
                    } else {
                        new_paragraph.len() - (IDEAL_PARAGRAPH_LENGTH - last_match - 3)
                    };

                    new_paragraph = new_paragraph.chars().take(take_chars_len).collect();
                    new_paragraph.push_str("...");

                    if new_paragraph.len() < IDEAL_PARAGRAPH_LENGTH {
                        return (
                            new_paragraph,
                            matched_indices
                                .iter()
                                .map(|index| *index - index_offset)
                                .collect(),
                        );
                    }
                }
            }
            _ => {}
        }

        // for index in 0..matched_indices.len() {
        //     if index == 0 {
        //         continue
        //     }
        //
        //     distance_between_indcies[index - 1] = matched_indices[index - 1] - matched_indices[index];
        // }

        if new_paragraph.len() > MAX_PARAGRAPH_LENGTH {
            new_paragraph = new_paragraph.chars().take(MAX_PARAGRAPH_LENGTH).collect();

            (
                new_paragraph,
                matched_indices
                    .iter()
                    .map(|index| *index - index_offset)
                    .filter(|index| *index < MAX_PARAGRAPH_LENGTH)
                    .collect(),
            )
        } else {
            (
                new_paragraph,
                matched_indices
                    .iter()
                    .map(|index| *index - index_offset)
                    .collect(),
            )
        }
    }
}

const MAX_PARAGRAPH_LENGTH: usize = 400;
const IDEAL_PARAGRAPH_LENGTH: usize = 100;
const PARAGRAPH_SLICE_PADDING: usize = 8;

#[derive(Clone)]
pub enum SearchRequest {
    Search { input: String },
    EndSearch,
    StopCurrentSearch,
}

pub enum SearchResult {
    Error(UnexpectedError),
    FileNameMatch { id: Uuid, path: String, matched_indices: Vec<usize>, score: i64 },
    FileContentMatches { id: Uuid, path: String, content_matches: Vec<ContentMatch> },
    NoMatch,
}

#[derive(Serialize)]
pub struct ContentMatch {
    paragraph: String,
    matched_indices: Vec<usize>,
    score: i64,
}
