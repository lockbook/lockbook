use std::cmp::Ordering;

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
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut possibs = Vec::new();
    for f in file_service::get_all_not_deleted_metadata(config, RepoSource::Local)? {
        possibs.push((f.id, path_service::get_path_by_id(config, f.id)?));
    }

    let root_name = file_service::get_root(config)?.decrypted_name;
    let matcher = SkimMatcherV2::default();

    let mut results = possibs
        .into_iter()
        .filter_map(|(id, path)| {
            let path = path.strip_prefix(&root_name).unwrap_or(&path).to_string();

            matcher
                .fuzzy_match(&path, input)
                .map(|score| SearchResultItem { id, path, score })
        })
        .collect::<Vec<SearchResultItem>>();

    results.sort_by(|a, b| match a.score.cmp(&b.score) {
        Ordering::Greater => Ordering::Less,
        Ordering::Less => Ordering::Greater,
        Ordering::Equal => {
            let chars1: Vec<char> = a.path.chars().collect();
            let chars2: Vec<char> = b.path.chars().collect();

            let n_chars1 = chars1.len();
            let n_chars2 = chars2.len();

            for i in 0..std::cmp::min(n_chars1, n_chars2) {
                let ord = chars1[i].cmp(&chars2[i]);
                if ord != Ordering::Equal {
                    return ord;
                }
            }

            n_chars1.cmp(&n_chars2)
        }
    });

    Ok(results)
}
