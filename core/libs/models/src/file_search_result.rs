use std::cmp::Ordering;

#[derive(Eq)]
pub struct FileSearchResult {
    pub name: String,
    pub score: i64,
}

impl Ord for FileSearchResult {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score.cmp(&other.score)
    }
}

impl PartialOrd for FileSearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for FileSearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}
