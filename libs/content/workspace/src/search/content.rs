use egui::Context;
use lb_rs::blocking::Lb;
use lb_rs::search::{ContentSearcher, SearchResult};

use crate::search::{SearchExecutor, SearchType};

pub struct ContentSearch {
    searcher: ContentSearcher,
    submitted_query: String,
}

impl ContentSearch {
    pub fn new(lb: &Lb, _ctx: &Context) -> Self {
        ContentSearch { searcher: lb.content_searcher(), submitted_query: String::new() }
    }
}

impl SearchExecutor for ContentSearch {
    fn search_type(&self) -> SearchType {
        SearchType::Content
    }

    fn handle_query(&mut self, query: &str) {
        if self.submitted_query == query {
            return;
        }
        self.submitted_query = query.to_string();
        self.searcher.query(query);
    }

    fn results(&self) -> &[SearchResult] {
        self.searcher.results()
    }
}
