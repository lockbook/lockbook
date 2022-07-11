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
use serde::{Deserialize, Serialize};


const DEBOUNCE_MILLIS: u64 = 150;
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

        Ok(thread::spawn(move || Self::search(results_tx, search_rx, Arc::new(ids), Arc::new(paths), Arc::new(files_contents))))
    }

    pub fn search(results_tx: Sender<SearchResult>, search_rx: Receiver<SearchRequest>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>, files_contents: Arc<HashMap<Uuid, String>>) {
        let mut last_search = match search_rx.recv() {
            Ok(last_search) => last_search,
            Err(_) => return
        };

        let should_continue = Arc::new(Mutex::new(true));

        {
            let input = match &last_search {
                SearchRequest::Search { input } => input.clone(),
                SearchRequest::EndSearch => return
            };

            spawn_search(results_tx.clone(), ids.clone(), paths.clone(), files_contents.clone(), should_continue.clone(), input);
        }

        let mut skip_channel_check = false;

        loop {
            if !skip_channel_check {
                last_search = match search_rx.recv() {
                    Ok(last_search) => last_search,
                    Err(_) => return
                };
            } else {
                skip_channel_check = false;
            }

            match should_continue.lock() {
                Ok(mut should_continue) => *should_continue = false,
                Err(e) => {
                    if let Err(_) = results_tx.send(SearchResult::Error(UnexpectedError(format!("{:?}", e)))) {
                        return
                    }
                }
            }

            thread::sleep(Duration::from_millis(DEBOUNCE_MILLIS));
            let current_search = search_rx.try_recv().ok();
            if let Some(search) = current_search {
                last_search = search;
                skip_channel_check = true;
                continue
            }

            let input = match &last_search {
                SearchRequest::Search { input } => input.clone(),
                SearchRequest::EndSearch => return
            };

            let should_continue = Arc::new(Mutex::new(true));
            spawn_search(results_tx.clone(), ids.clone(), paths.clone(), files_contents.clone(), should_continue.clone(), input);
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
    search_file_names(results_tx.clone(), should_continue.clone(), ids.clone(), paths.clone(), &input)?;
    if *should_continue.lock().map_err(|e| UnexpectedError(format!("{:?}", e)))? {
        search_file_contents(results_tx, should_continue, ids, paths.clone(), files_contents, &input)?;
    }

    Ok(())
}

pub fn search_file_names(results_tx: Sender<SearchResult>, should_continue: Arc<Mutex<bool>>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>, search: &str) -> Result<(), UnexpectedError> {
    for id in ids.as_ref() {
        if !*should_continue.lock().map_err(|e| UnexpectedError(format!("{:?}", e)))? {
            return Ok(())
        }

        if let Some(fuzzy_match) = FuzzySearch::new(search, &paths[id]).case_insensitive().best_match() {
            if fuzzy_match.score() >= LOWEST_SCORE_THRESHOLD {
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

pub fn search_file_contents(results_tx: Sender<SearchResult>, should_continue: Arc<Mutex<bool>>, ids: Arc<Vec<Uuid>>, paths: Arc<HashMap<Uuid, String>>, files_contents: Arc<HashMap<Uuid, String>>, search: &str) -> Result<(), UnexpectedError> {
    for id in ids.as_ref() {
        if !*should_continue.lock().map_err(|e| UnexpectedError(format!("{:?}", e)))? {
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
}

#[derive(Serialize, Deserialize)]
pub struct ContentMatch {
    paragraph: String,
    matched_indices: Vec<usize>,
    score: isize
}


