use crate::model::repo::RepoSource;
use crate::{CoreError, RequestContext, UnexpectedError};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use itertools::Itertools;
use sublime_fuzzy::FuzzySearch;
use uuid::Uuid;
use lockbook_models::file_metadata::{DecryptedFiles, FileType};
use serde::{Serialize};


const DEBOUNCE_MILLIS: u64 = 100;
const LOWEST_SCORE_THRESHOLD: isize = 150;

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

    pub fn start_search(&mut self, results_tx: Sender<SearchResult>, search_rx: Receiver<SearchRequest>) -> Result<JoinHandle<()>, CoreError> {
        let files_not_deleted: DecryptedFiles = self.get_all_not_deleted_metadata(RepoSource::Local)?;

        let mut ids: Vec<Uuid> = Vec::new();
        let mut paths: HashMap<Uuid, String> = HashMap::new();
        let mut files_contents: HashMap<Uuid, String> = HashMap::new();

        for (id, file) in &files_not_deleted {
            if file.file_type == FileType::Document && (file.decrypted_name.as_str().ends_with(".txt") || file.decrypted_name.as_str().ends_with(".md")) {
                ids.push(*id);
                paths.insert(file.id, Self::path_by_id_helper(&files_not_deleted, file.id)?);
                files_contents.insert(file.id, String::from(String::from_utf8_lossy(&self.read_document(self.config, RepoSource::Local, file.id)?)));
            }
        }

        Ok(thread::spawn(move || {
            if let Err(e) = Self::search(&results_tx, search_rx, Arc::new(ids), Arc::new(paths), Arc::new(files_contents)) {
                if let Err(_) = results_tx.send(SearchResult::Error(UnexpectedError(format!("{:?}", e)))) {
                    // can't send the error, so nothing to do
                }
            }
        }))
    }

    pub fn search(results_tx: &Sender<SearchResult>, search_rx: Receiver<SearchRequest>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>, files_contents: Arc<HashMap<Uuid, String>>) -> Result<(), UnexpectedError> {
        let mut last_search = match search_rx.recv() {
            Ok(last_search) => last_search,
            Err(_) => return Ok(())
        };

        let should_continue = Arc::new(Mutex::new(true));

        match &last_search {
            SearchRequest::Search { input } => {
                spawn_search(results_tx.clone(), ids.clone(), paths.clone(), files_contents.clone(), should_continue.clone(), input.clone());
            },
            SearchRequest::EndSearch => return Ok(()),
            SearchRequest::StopCurrentSearch => {
                // no search going on
            }
        };

        let mut skip_channel_check = false;

        loop {
            if !skip_channel_check {
                last_search = match search_rx.recv() {
                    Ok(last_search) => last_search,
                    Err(_) => return Ok(())
                };
            } else {
                skip_channel_check = false;
            }

            *should_continue.lock()? = false;

            thread::sleep(Duration::from_millis(DEBOUNCE_MILLIS));
            let current_search = search_rx.try_recv().ok();
            if let Some(search) = current_search {
                last_search = search;
                skip_channel_check = true;
                continue
            }

            match &last_search {
                SearchRequest::Search { input } => {
                    let should_continue = Arc::new(Mutex::new(true));
                    spawn_search(results_tx.clone(), ids.clone(), paths.clone(), files_contents.clone(), should_continue.clone(), input.clone());
                },
                SearchRequest::EndSearch => return Ok(()),
                SearchRequest::StopCurrentSearch => {
                    *should_continue.lock()? = false;
                }
            };

        }
    }
}

pub fn spawn_search(results_tx: Sender<SearchResult>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>, files_contents: Arc<HashMap<Uuid, String>>, should_continue: Arc<Mutex<bool>>, input: String) {
    thread::spawn(move || {
        if let Err(e) = search_loop(results_tx.clone(), ids, paths, files_contents, should_continue, input) {
            if let Err(_) = results_tx.send(SearchResult::Error(e)) {
                // can't send the error, so nothing to do
            }
        }
    });
}

pub fn search_loop(results_tx: Sender<SearchResult>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>, files_contents: Arc<HashMap<Uuid, String>>, should_continue: Arc<Mutex<bool>>, input: String) -> Result<(), UnexpectedError> {
    let mut no_matches = true;

    search_file_names(&results_tx, &should_continue, &ids, &paths, &input, &mut no_matches)?;
    if *should_continue.lock()? {
        search_file_contents(&results_tx, &should_continue, &ids, &paths, &files_contents, &input, &mut no_matches)?;
    }

    if no_matches && *should_continue.lock()? {
        if let Err(_) = results_tx.send(SearchResult::NoMatch) {
            // session already ended so no point
        }
    }

    Ok(())
}

pub fn search_file_names(results_tx: &Sender<SearchResult>, should_continue: &Arc<Mutex<bool>>, ids: &Arc<Vec<Uuid>>, paths: &Arc<HashMap<Uuid, String>>, search: &str, no_matches: &mut bool) -> Result<(), UnexpectedError> {
    for id in ids.as_ref() {
        if !*should_continue.lock()? {
            return Ok(())
        }

        if let Some(fuzzy_match) = FuzzySearch::new(search, &paths[id]).case_insensitive().best_match() {
            if fuzzy_match.score() >= LOWEST_SCORE_THRESHOLD {
                if *no_matches {
                    *no_matches = false;
                }

                if let Err(_) = results_tx.send(SearchResult::FileNameMatch {
                    id: id.clone(),
                    path: paths[id].clone(),
                    matched_indices: fuzzy_match.matched_indices().map(|ind| *ind).collect_vec(),
                    score: fuzzy_match.score()
                }) {
                    break
                }
            }
        }
    }

    Ok(())
}

pub fn search_file_contents(results_tx: &Sender<SearchResult>, should_continue: &Arc<Mutex<bool>>, ids: &Arc<Vec<Uuid>>, paths: &Arc<HashMap<Uuid, String>>, files_contents: &Arc<HashMap<Uuid, String>>, search: &str, no_matches: &mut bool) -> Result<(), UnexpectedError> {
    for id in ids.as_ref() {
        if !*should_continue.lock()? {
            return Ok(())
        }

        let paragraphs = files_contents[id].split("\n");
        let mut content_matches: Vec<ContentMatch> = Vec::new();

        for paragraph in paragraphs {
            if let Some(fuzzy_match) = FuzzySearch::new(search, paragraph).case_insensitive().best_match() {
                if fuzzy_match.score() >= LOWEST_SCORE_THRESHOLD {
                    content_matches.push(ContentMatch { paragraph: paragraph.to_string(), matched_indices: fuzzy_match.matched_indices().map(|ind| *ind).collect_vec(), score: fuzzy_match.score()});
                }
            }
        }

        if !content_matches.is_empty() {
            if *no_matches {
                *no_matches = false;
            }

            if let Err(_) = results_tx.send(SearchResult::FileContentMatches {
                id: id.clone(),
                path: paths[id].clone(),
                content_matches,
            }) {
                break
            }
        }
    }

    Ok(())
}

#[derive(Clone)]
pub enum SearchRequest {
    Search {
        input: String
    },
    EndSearch,
    StopCurrentSearch,
}

pub enum SearchResult {
    Error(UnexpectedError),
    FileNameMatch {
        id: Uuid,
        path: String,
        matched_indices: Vec<usize>,
        score: isize
    },
    FileContentMatches {
        id: Uuid,
        path: String,
        content_matches: Vec<ContentMatch>,
    },
    NoMatch
}

#[derive(Serialize)]
pub struct ContentMatch {
    paragraph: String,
    matched_indices: Vec<usize>,
    score: isize
}


