#[derive(Clone)]
pub struct ContentSearch {
    doc_store: Arc<RwLock<DocStore>>,
    query_state: Arc<RwLock<QueryState>>,
}

impl ContentSearch {
    pub fn new(lb: &Lb, ctx: &Context) -> Self {
        let content_search =
            ContentSearch { doc_store: Default::default(), query_state: Default::default() };

        let lb = lb.clone();
        let ctx = ctx.clone();
        let bg_cs = content_search.clone();
        thread::spawn(move || {
            bg_cs.build_doc_store(lb, ctx);
        });

        content_search
    }
}

#[derive(Default)]
pub struct DocStore {
    documents: Vec<(File, String, String)>,

    uningested_files: Vec<File>,
    ignored_ids: usize,
    ingest_failures: usize,

    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

// scanning things
impl ContentSearch {
    fn build_doc_store(&self, lb: Lb, ctx: Context) {
        let start = Instant::now();

        let metas = lb.list_metadatas().unwrap();
        let paths = Arc::new(lb.list_paths_with_ids(None).unwrap());

        self.doc_store.write().unwrap().start_time = Some(start);

        for meta in metas {
            let mut ignore = false;
            if !meta.is_document() {
                continue;
            }

            if !meta.name.ends_with(".md") {
                ignore = true;
            }

            if ignore {
                self.doc_store.write().unwrap().ignored_ids += 1;
            } else {
                self.doc_store.write().unwrap().uningested_files.push(meta);
            }
        }

        for _ in 0..available_parallelism()
            .map(|number| number.get())
            .unwrap_or(4)
        {
            let bg_ds = self.doc_store.clone();
            let bg_lb = lb.clone();
            let ctx = ctx.clone();
            let bg_paths = paths.clone();
            thread::spawn(move || {
                loop {
                    // thread::sleep(Duration::from_secs(1));
                    let Some(meta) = bg_ds.write().unwrap().uningested_files.pop() else {
                        return;
                    };

                    let id = meta.id;
                    let doc = bg_lb
                        .read_document(meta.id, false)
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok());

                    let mut doc_store = bg_ds.write().unwrap();
                    if let Some(mut doc) = doc {
                        // todo: see lowercasing notes
                        let doc = doc.to_lowercase();
                        doc_store.documents.push((
                            meta,
                            bg_paths.iter().find(|(i, _)| *i == id).unwrap().1.clone(),
                            doc,
                        ));
                    } else {
                        doc_store.ingest_failures += 1;
                    }

                    if doc_store.uningested_files.is_empty() {
                        doc_store.end_time = Some(Instant::now());
                    }
                    ctx.request_repaint();
                }
            });
        }
    }
}

#[derive(Default)]
pub struct QueryState {
    submitted_query: String,
    ellapsed_ms: u128,
    matches: Vec<Matches>,
}

#[derive(Default)]
pub struct Matches {
    id: Uuid,
    highlights: Vec<Range<usize>>,

    exact_matches: u32,
    substring_matches: u32,
}

impl SearchExecutor for ContentSearch {
    fn search_type(&self) -> super::SearchType {
        SearchType::Content
    }

    fn handle_query(&mut self, query: &str) {
        // todo: expose lowercase controls. Right now lowercasing is done through a path of least
        // resistance. We lowercase the documents during ingestion (in place) which has a minimal
        // impact on performance. Doing it within this function makes the query take 2x as long.
        // the optimal solution would use a FSM and have controls for managing cases.
        //
        // also there seem to be some fancy algorithms in the space and it could be worth exploring
        // them as well
        let query = query.to_ascii_lowercase();
        let start = Instant::now();

        let docs = self.doc_store.read().unwrap();
        let mut results = self.query_state.write().unwrap();
        if results.submitted_query == query {
            return;
        }

        results.submitted_query = query.to_string();

        // todo: incrementalism
        results.matches.clear();

        if query.is_empty() {
            return;
        }

        for (meta, _, content) in &docs.documents {
            let mut m = Matches { id: meta.id, ..Default::default() };

            for (idx, _) in content.match_indices(&query) {
                m.highlights.push(idx..idx + query.len());
                m.exact_matches += 1;
            }

            let mut all_words_matched = true;
            for sub_query in query.split_whitespace() {
                let mut sub_query_matched = false;
                for (idx, _) in content.match_indices(sub_query) {
                    sub_query_matched = true;
                    if m.highlights.iter().any(|range| range.contains(&idx)) {
                        continue;
                    }
                    m.highlights.push(idx..idx + sub_query.len());
                    m.substring_matches += 1;
                }
                if !sub_query_matched {
                    all_words_matched = false;
                }
            }

            if all_words_matched {
                results.matches.push(m);
            }
        }

        results.matches.sort_unstable_by(|a, b| {
            if a.exact_matches > 0 || b.exact_matches > 0 {
                b.exact_matches.cmp(&a.exact_matches)
            } else {
                b.substring_matches.cmp(&a.substring_matches)
            }
        });
        // match multiple parts of a query
        // score exact matches higher than other types of matches
        // then sort by match count
        // dedup exact matches and partial matches

        results.ellapsed_ms = start.elapsed().as_millis();
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) -> Option<lb_rs::Uuid> {
        ui.set_min_size(ui.available_size());
        let doc_store = self.doc_store.read().unwrap();
        let query_state = self.query_state.read().unwrap();

        ui.vertical(|ui| {
            // results
            ui.vertical(|ui| {
                // todo: sort
                for m in &query_state.matches {
                    ui.label(format!(
                        "{}: {} exact matches, {} sub matches",
                        doc_store
                            .documents
                            .iter()
                            .find(|(f, _, _)| f.id == m.id)
                            .unwrap()
                            .0
                            .name,
                        m.exact_matches,
                        m.substring_matches
                    ));
                }
            });

            // info card
            ui.vertical_centered(|ui| {
                ui.label(format!(
                    "Read {} of {} documents.",
                    doc_store.documents.len(),
                    doc_store.uningested_files.len()
                        + doc_store.documents.len()
                        + doc_store.ignored_ids
                        + doc_store.ingest_failures
                ));

                ui.label(format!(
                    "{} documents ignored. {} errors.",
                    doc_store.ignored_ids, doc_store.ingest_failures
                ));

                if let (Some(start), Some(end)) = (doc_store.start_time, doc_store.end_time) {
                    ui.label(format!("Search Index prepared in {}ms", (end - start).as_millis()));
                }

                if !query_state.submitted_query.is_empty() {
                    ui.label(format!("Query processed in {}ms", query_state.ellapsed_ms));
                }
            });
        });

        None
    }

    fn show_preview(&mut self, ui: &mut egui::Ui) {}
}

use std::{
    collections::HashMap,
    ops::{Not, Range},
    sync::{Arc, RwLock},
    thread::{self, available_parallelism},
    time::{Duration, Instant},
};

use egui::Context;
use lb_rs::{Uuid, blocking::Lb, model::file::File};

use crate::{
    search::{SearchExecutor, SearchType},
    workspace::Workspace,
};
