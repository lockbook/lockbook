pub struct PathSearch {
    submitted_query: String,
    nucleo: Nucleo<PathResult>,
}

impl SearchExecutor for PathSearch {
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
            ui.spacing_mut().item_spacing.y = 2.0;

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
        // functionality:
        // todo: keyboard shortcut to open a result
        // todo: response that opens the tab
        // todo: support folders, and generally a richer icon experience
        let theme = ui.ctx().get_lb_theme();
        let name_color = theme.neutral_fg();
        let parent_color = theme.neutral_fg_secondary();

        // nucleo returns char indices into the full path; pass a char offset
        // so each sub-line filters the shared slice without allocating.
        let parent_path = entry.parent_path();
        let parent_char_len = parent_path.chars().count() as u32;

        Frame::new()
            .inner_margin(Margin::symmetric(8, 6))
            .corner_radius(CornerRadius::same(4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;

                    DocType::from_name(&entry.file.name).to_icon().show(ui);

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        Self::highlighted_line(
                            ui,
                            &entry.file.name,
                            &entry.highlight,
                            parent_char_len,
                            name_color,
                            14.0,
                        );
                        Self::highlighted_line(
                            ui,
                            parent_path,
                            &entry.highlight,
                            0,
                            parent_color,
                            11.0,
                        );
                    });
                });
            });
    }

    /// Render `text` as a single laid-out line, bolding any character whose
    /// char-index (plus `char_offset`) appears in `highlights`. Background
    /// color is reserved for the current selection.
    fn highlighted_line(
        ui: &mut Ui, text: &str, highlights: &[u32], char_offset: u32, color: egui::Color32,
        size: f32,
    ) {
        let regular = egui::FontId::new(size, egui::FontFamily::Proportional);
        let bold = egui::FontId::new(size, egui::FontFamily::Name(Arc::from("Bold")));
        let mut job = egui::text::LayoutJob::default();
        for (i, c) in text.chars().enumerate() {
            let hi = highlights.contains(&(char_offset + i as u32));
            let fmt = egui::TextFormat {
                font_id: if hi { bold.clone() } else { regular.clone() },
                color,
                ..Default::default()
            };
            job.append(&c.to_string(), 0.0, fmt);
        }
        ui.label(job);
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

use egui::{Context, CornerRadius, Frame, Margin, RichText, Ui};
use lb_rs::{blocking::Lb, model::file::File};
use nucleo::{
    Matcher, Nucleo,
    pattern::{CaseMatching, Normalization},
};

use crate::{
    search::{SearchExecutor, SearchType},
    show::DocType,
    theme::palette_v2::ThemeExt,
};
