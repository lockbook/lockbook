use std::cmp::Ordering;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use uuid::Uuid;

use crate::file_service;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::path_service;
use crate::CoreError;

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

pub fn search_file_paths(config: &Config, input: &str) -> Result<Vec<SearchResultItem>, CoreError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let root_name = file_service::get_root(config)?.decrypted_name;
    let matcher = SkimMatcherV2::default();

    let mut results = Vec::new();
    for f in file_service::get_all_not_deleted_metadata(config, RepoSource::Local)? {
        let path = path_service::get_path_by_id(config, f.id)?;
        let path_without_root = path.strip_prefix(&root_name).unwrap_or(&path).to_string();

        if let Some(score) = matcher.fuzzy_match(&path_without_root, input) {
            results.push(SearchResultItem { id: f.id, path: path_without_root, score });
        }
    }
    results.sort();

    Ok(results)
}
