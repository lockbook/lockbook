use crate::model::repo::RepoSource;
use crate::CoreError;
use crate::Tx;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::cmp::Ordering;
use uuid::Uuid;

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

impl Tx<'_> {
    pub fn search_file_paths(&self, input: &str) -> Result<Vec<SearchResultItem>, CoreError> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let root_name = self.root()?.decrypted_name;
        let matcher = SkimMatcherV2::default();

        let mut results = Vec::new();
        for f in self.get_all_not_deleted_metadata(RepoSource::Local)? {
            let path = self.get_path_by_id(f.0)?;
            let path_without_root = path.strip_prefix(&root_name).unwrap_or(&path).to_string();

            if let Some(score) = matcher.fuzzy_match(&path_without_root, input) {
                results.push(SearchResultItem { id: f.0, path: path_without_root, score });
            }
        }
        results.sort();

        Ok(results)
    }
}
