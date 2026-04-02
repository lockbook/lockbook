pub struct PathSearch {
    submitted_query: String,
    nucleo: Nucleo<PathResult>,
}

impl SearhExecutor for PathSearch {
    fn search_type(&self) -> super::SearchType {
        SearchType::Path
    }

    fn handle_query(&mut self, query: &str) {
        if self.submitted_query != query {
            self.nucleo.pattern.reparse(
                0,
                query,
                CaseMatching::Smart,
                Normalization::Smart,
                self.submitted_query.starts_with(query),
            );
            self.submitted_query = query.to_string();
        }
        self.nucleo.tick(1);
    }

    fn show_result_picker(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            let snapshot = self.nucleo.snapshot();
            let mut matcher = Matcher::new(nucleo::Config::DEFAULT);

            for item in snapshot.matched_items(0..snapshot.matched_item_count()) {
                let mut entry = item.data.clone();

                let mut indices = Vec::new();

                self.nucleo.pattern.column_pattern(0).indices(
                    item.matcher_columns[0].slice(..),
                    &mut matcher,
                    &mut indices,
                );

                entry.highlight = indices;

                ui.spacing_mut().item_spacing.y = 5.0;
                self.show_result_cell(ui, &entry);
            }
        });
    }

    fn show_preview(&mut self, ui: &mut egui::Ui) {
        // todo!()
    }
}

impl PathSearch {
    pub fn new(lb: &Lb, ctx: &Context) -> Self {
        let metas = lb.list_metadatas().unwrap();
        // todo there may be gains to be had to retrieve FilePaths instead of id paths
        let mut id_paths = lb.list_paths_with_ids(None).unwrap();
        id_paths.retain(|(_, path)| path != "/");

        let ctx = ctx.clone();
        let notify = Arc::new(move || {
            ctx.request_repaint();
        });

        let nucleo = Nucleo::new(nucleo::Config::DEFAULT, notify, None, 1);
        let injector = nucleo.injector();

        for (id, path) in id_paths {
            injector.push(
                PathResult {
                    file: metas.iter().find(|m| m.id == id).unwrap().clone(),
                    path: path.clone(),
                    highlight: vec![],
                },
                |e, cols| {
                    cols[0] = e.path.as_str().into();
                },
            );
        }

        Self { submitted_query: Default::default(), nucleo }
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
}

#[derive(Clone)]
pub struct PathResult {
    file: File,
    path: String,
    highlight: Vec<u32>,
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

use std::sync::Arc;

use egui::{Context, Ui};
use lb_rs::{Uuid, blocking::Lb, model::file::File};
use nucleo::{
    pattern::{CaseMatching, Normalization}, Matcher, Nucleo
};

use crate::{
    search::{SearchType, SearhExecutor},
    show::DocType,
};
