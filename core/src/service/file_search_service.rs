use std::collections::binary_heap::BinaryHeap;

use crate::model::state::Config;
use crate::{CoreError, list_paths};

use lockbook_models::file_search_result::FileSearchResult;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

struct SearchState {
    matcher: SkimMatcherV2,
    possibs: Vec<String>,
}

impl SearchState {
    fn new(config: &Config) -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
            possibs: list_paths(config, None).unwrap_or_default(),
        }
    }

    fn find_and_rank(&self, pattern: &str) -> Vec<FileSearchResult> {
        let mut heap = BinaryHeap::new();

        for path in &self.possibs {
            if let Some(score) = self.matcher.fuzzy_match(&path, &pattern) {
                heap.push(FileSearchResult { name: path.clone(), score: score });
            }
        }

        heap.into_iter().collect()
    }
}

pub fn fuzzy_search_all_paths(config: &Config, string: &str) -> Result<Vec<FileSearchResult>, CoreError> {
    let state = SearchState::new(config);
    Ok(state.find_and_rank(string))
}
