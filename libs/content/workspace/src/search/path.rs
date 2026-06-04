use egui::Context;
use lb_rs::blocking::Lb;
use lb_rs::search::{PathSearcher, SearchResult};

use crate::search::{SearchExecutor, SearchType};

pub struct PathSearch {
    searcher: PathSearcher,
}

impl PathSearch {
    pub fn new(lb: &Lb, _ctx: &Context) -> Self {
        Self { searcher: lb.path_searcher() }
    }
}

impl SearchExecutor for PathSearch {
    fn search_type(&self) -> SearchType {
        SearchType::Path
    }

    fn handle_query(&mut self, query: &str) {
        self.searcher.query(query);
    }

    fn results(&self) -> &[SearchResult] {
        self.searcher.results()
    }
}
