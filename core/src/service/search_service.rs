use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use uuid::Uuid;

use crate::file_service;
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::path_service;
use crate::CoreError;

#[derive(Debug)]
pub struct SearchResultItem {
    pub id: Uuid,
    pub path: String,
    pub score: i64,
}

pub fn search_file_paths(config: &Config, input: &str) -> Result<Vec<SearchResultItem>, CoreError> {
    let mut possibs = Vec::new();
    for f in file_service::get_all_not_deleted_metadata(config, RepoSource::Local)? {
        possibs.push((f.id, path_service::get_path_by_id(config, f.id)?));
    }

    let matcher = SkimMatcherV2::default();

    let mut results = possibs
        .into_iter()
        .filter_map(|(id, path)| {
            matcher
                .fuzzy_match(&path, &input)
                .and_then(|score| Some(SearchResultItem { id, path, score }))
        })
        .collect::<Vec<SearchResultItem>>();

    results.sort_by(|a, b| a.score.cmp(&b.score));

    Ok(results)
}
