use crate::model::repo::RepoSource;
use crate::{CoreError, RequestContext};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use sublime_fuzzy::FuzzySearch;
use uuid::Uuid;
use lockbook_models::file_metadata::{DecryptedFileMetadata, FileType};

const DEBOUNCE_MILLIS: u64 = 100;

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
        let files: Vec<DecryptedFileMetadata> = self.get_all_not_deleted_metadata(RepoSource::Local)?
            .into_values()
            .filter(|file| file.file_type == FileType::Document && (file.decrypted_name.as_str().ends_with(".txt") || file.decrypted_name.as_str().ends_with(".md")))
            .collect();

        let mut files_contents: HashMap<Uuid, String> = HashMap::new();

        for file in &files {
            files_contents.insert(file.id, String::from(String::from_utf8_lossy(&self.read_document(self.config, RepoSource::Local, file.id)?)));
        }

        Ok(thread::spawn(move || Self::search(results_tx, search_rx, Arc::new(files), Arc::new(files_contents))))
    }

    pub fn search(results_tx: Sender<SearchResult>, search_rx: Receiver<SearchRequest>, files: Arc<Vec<DecryptedFileMetadata>>, files_contents: Arc<HashMap<Uuid, String>>) {
        let mut last_search = search_rx.recv().unwrap();
        let should_continue = Arc::new(Mutex::new(true));

        {
            let input = match &last_search {
                SearchRequest::Search { input } => input.clone(),
                SearchRequest::EndSearch => return
            };

            spawn_search(results_tx.clone(), files.clone(), files_contents.clone(), should_continue.clone(), input);
        }


        let mut skip_channel_check = false;

        loop {
            if !skip_channel_check {
                last_search = search_rx.recv().unwrap();
            } else {
                skip_channel_check = false;
            }
            *should_continue.lock().unwrap() = false;

            thread::sleep(Duration::from_millis(DEBOUNCE_MILLIS));
            let current_search = search_rx.try_recv().ok();
            if let Some(search) = current_search {
                last_search = search;
                skip_channel_check = true;
                continue
            }

            let input = match &last_search {
                SearchRequest::Search { input } => input.clone(),
                SearchRequest::EndSearch => {
                    results_tx.send(SearchResult::End).unwrap();
                    return
                }
            };

            let should_continue = Arc::new(Mutex::new(true));
            spawn_search(results_tx.clone(), files.clone(), files_contents.clone(), should_continue.clone(), input);
        }
    }
}

pub fn spawn_search(results_tx: Sender<SearchResult>, files: Arc<Vec<DecryptedFileMetadata>>, files_contents: Arc<HashMap<Uuid, String>>, should_continue: Arc<Mutex<bool>>, input: String) {
    *should_continue.lock().unwrap() = true;

    thread::spawn(move || {
        search_file_names(results_tx.clone(), should_continue.clone(), files.clone(), &input);
        if *should_continue.lock().unwrap() {
            search_file_contents(results_tx, should_continue, files, files_contents, &input);
        }
    });
}

pub fn search_file_names(results_tx: Sender<SearchResult>, should_continue: Arc<Mutex<bool>>, files: Arc<Vec<DecryptedFileMetadata>>, search: &str) {
    for file in files.as_ref() {
        if !*should_continue.lock().unwrap() {
            return
        }

        if let Some(fuzzy_match) = FuzzySearch::new(search, &file.decrypted_name).case_insensitive().best_match() {
            results_tx.send(SearchResult::FileNameMatch {
                id: file.id,
                name: file.decrypted_name.clone(),
                score: fuzzy_match.score()
            }).unwrap();
        }
    }
}

pub fn search_file_contents(results_tx: Sender<SearchResult>, should_continue: Arc<Mutex<bool>>, files: Arc<Vec<DecryptedFileMetadata>>, files_contents: Arc<HashMap<Uuid, String>>, search: &str) {
    for file in files.as_ref() {
        if !*should_continue.lock().unwrap() {
            return
        }

        let content = files_contents.get(&file.id).unwrap();

        if let Some(fuzzy_match) = FuzzySearch::new(search, &content).case_insensitive().best_match() {
            // fuzzy_match.matched_indices().next().unwrap()
            results_tx.send(SearchResult::FileContentMatch {
                id: file.id,
                file_name: file.decrypted_name.clone(),
                content: "".to_string(),
                score: fuzzy_match.score()
            }).unwrap();
        }
    }
}

#[derive(Clone)]
pub enum SearchRequest {
    Search {
        input: String
    },
    EndSearch,
}

pub enum SearchResult {
    Error(CoreError),
    FileNameMatch {
        id: Uuid,
        name: String,
        score: isize
    },
    FileContentMatch {
        id: Uuid,
        file_name: String,
        content: String,
        score: isize
    },
    End
}


