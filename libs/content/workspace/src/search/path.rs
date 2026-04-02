pub struct PathSearch {
    metas: Vec<File>,
    id_paths: Vec<(Uuid, String)>,
    results: Vec<PathResult>,
}

impl SearhExecutor for PathSearch {
    fn search_type(&self) -> super::SearchType {
        SearchType::Path
    }

    fn handle_query(&mut self, query: &str) {
        let mut results = self.path_candidates(query);
        self.score_paths(&mut results);

        results.sort_by_key(|r| -r.score);

        if let Some(result) = self.id_match(query) {
            results.insert(0, result);
        }

        self.results = results;
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            for item in &self.results {
                ui.spacing_mut().item_spacing.y = 5.0;
                self.show_result_cell(ui, item);
            }
        });
    }

    fn show_preview(&mut self, ui: &mut egui::Ui) {
        // todo!()
    }
}

impl PathSearch {
    pub fn new(lb: &Lb) -> Self {
        let metas = lb.list_metadatas().unwrap();
        // todo there may be gains to be had to retrieve FilePaths instead of id paths
        let mut id_paths = lb.list_paths_with_ids(None).unwrap();
        id_paths.retain(|(_, path)| path != "/");

        Self { metas, id_paths, results: vec![] }
    }

    fn show_result_cell(&self, ui: &mut Ui, entry: &PathResult) {
        ui.horizontal(|ui| {
            // todo: aesthetics, spacing, background color, vertical centering
            // todo: support folders, and generally a richer icon experience
            DocType::from_name(&entry.path).to_icon().show(ui);

            ui.vertical(|ui| {
                ui.label(&entry.file.name);
                ui.label(entry.parent_path());
            })
        });
    }

    fn path_candidates(&self, query: &str) -> Vec<PathResult> {
        let mut search_results = vec![];

        for (id, path) in &self.id_paths {
            let mut highlight = vec![];

            let mut query_iter = query.chars().rev();
            let mut current_query_char = query_iter.next();

            for (path_ind, path_char) in path.char_indices().rev() {
                if let Some(qc) = current_query_char {
                    if qc.eq_ignore_ascii_case(&path_char) {
                        highlight.push(path_ind);
                        current_query_char = query_iter.next();
                    }
                } else {
                    break;
                }
            }

            if current_query_char.is_none() {
                search_results.push(PathResult {
                    file: self.metas.iter().find(|f| f.id == *id).unwrap().clone(),
                    path: path.clone(),
                    highlight,
                    score: 0,
                });
            }
        }
        search_results
    }

    fn score_paths(&self, candidates: &mut [PathResult]) {
        // tunable bonuses for path search
        let smaller_paths = 10;
        let suggested = 10;
        let filename = 30;
        let editable = 3;

        candidates.sort_by_key(|a| a.path.len());

        // the 10 smallest paths start with a mild advantage
        for i in 0..smaller_paths {
            if let Some(candidate) = candidates.get_mut(i) {
                candidate.score = (smaller_paths - i) as i64;
            }
        }

        // items in suggested docs have their score boosted
        // not sure I believe in this, we can restore later
        // for cand in candidates.iter_mut() {
        //     if self.suggested_docs.contains(&cand.id()) {
        //         if let SearchResult::PathMatch { id: _, path: _, matched_indices: _, score } = cand
        //         {
        //             *score += suggested;
        //         }
        //     }
        // }

        // to what extent is the match in the name of the file
        for cand in candidates.iter_mut() {
            let mut name_match = 0;
            let mut name_size = 0;

            for (i, c) in cand.path.char_indices().rev() {
                if c == '/' {
                    break;
                }
                name_size += 1;
                if cand.highlight.contains(&i) {
                    name_match += 1;
                }
            }

            let match_portion = name_match as f32 / name_size.max(1) as f32;
            cand.score += (match_portion * filename as f32) as i64;
        }

        // if this document is editable in platform
        for cand in candidates.iter_mut() {
            if cand.path.ends_with(".md") || cand.path.ends_with(".svg") {
                cand.score += editable;
            }
        }
    }

    fn id_match(&self, query: &str) -> Option<PathResult> {
        if query.len() < 8 {
            return None;
        }

        let query = if query.starts_with("lb://") {
            query.replacen("lb://", "", 1)
        } else {
            query.to_string()
        };

        for (id, path) in &self.id_paths {
            if id.to_string().contains(&query) {
                return Some(PathResult {
                    file: self.metas.iter().find(|f| f.id == *id).unwrap().clone(),
                    path: path.clone(),
                    highlight: vec![],
                    score: 100,
                });
            }
        }

        None
    }
}

pub struct PathResult {
    file: File,
    path: String,
    highlight: Vec<usize>,
    score: i64,
}

impl PathResult {
    fn parent_path(&self) -> &str {
        if self.path.ends_with('/') {
            self.path
                .strip_suffix(&format!("{}/", self.file.name))
                .unwrap()
        } else {
            self.path.strip_suffix(&self.file.name).unwrap()
        }
    }
}

use egui::Ui;
use lb_rs::{Uuid, blocking::Lb, model::file::File};

use crate::{
    search::{SearchType, SearhExecutor},
    show::DocType,
};
