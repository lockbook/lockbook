use std::cmp::Ordering;
use std::fmt;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::Uuid;

pub struct Searcher {
    paths: Vec<(Uuid, String)>,
    matcher: SkimMatcherV2,
}

impl Searcher {
    pub fn new(paths: Vec<(Uuid, String)>) -> Self {
        let matcher = SkimMatcherV2::default();

        Self { paths, matcher }
    }

    pub fn search(&self, input: &str) -> Vec<SearchResultItem> {
        if input.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for (id, path) in &self.paths {
            if let Some(score) = self.matcher.fuzzy_match(path, input) {
                results.push(SearchResultItem { id: *id, path: path.to_string(), score });
            }
        }
        results.sort();
        results
    }
}

impl fmt::Debug for Searcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Searcher")
            .field("paths", &self.paths)
            .finish()
    }
}

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
