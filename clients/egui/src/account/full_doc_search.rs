use eframe::egui;
use lb::blocking::Lb;
use lb::model::file::File;
use lb::service::search::{ContentMatch, SearchConfig, SearchResult};
use lb::Uuid;
use workspace_rs::theme::icons::Icon;
use workspace_rs::widgets::Button;

use crate::model::DocType;

pub struct FullDocSearch {
    pub query: String,
    pub results: Vec<SearchResult>,
}

impl FullDocSearch {
    const X_MARGIN: f32 = 15.0;

    pub fn new() -> Self {
        Self { query: String::new(), results: Vec::new() }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, core: &Lb) -> Option<Uuid> {
        ui.vertical_centered(|ui| {
            let output = egui::TextEdit::singleline(&mut self.query)
                .desired_width(ui.available_size_before_wrap().x - 5.0)
                .hint_text("Search")
                .margin(egui::vec2(Self::X_MARGIN, 9.0))
                .show(ui);

            let search_icon_width = 15.0; // approximation
            let is_text_clipped =
                output.galley.rect.width() + Self::X_MARGIN * 2.0 + search_icon_width
                    > output.response.rect.width();

            if !is_text_clipped {
                ui.allocate_ui_at_rect(output.response.rect, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);

                        if self.query.is_empty() {
                            Icon::SEARCH.color(egui::Color32::GRAY).show(ui);
                        } else {
                            ui.spacing_mut().button_padding = egui::vec2(0.0, 0.0);
                            if Button::default().icon(&Icon::CLOSE).show(ui).clicked() {
                                self.query = "".to_string();
                                self.results = vec![];
                            }
                        }
                    });
                });
            };

            if output.response.changed() {
                self.results = core
                    .search(&self.query, SearchConfig::PathsAndDocuments)
                    .unwrap_or_default();
            }

            egui::ScrollArea::vertical()
                .show(ui, |ui| self.show_results(ui, core))
                .inner
        })
        .inner
    }

    pub fn show_results(&mut self, ui: &mut egui::Ui, core: &Lb) -> Option<Uuid> {
        ui.add_space(20.0);

        for sr in self.results.iter() {
            let sr_res = ui.vertical(|ui| {
                match sr {
                    SearchResult::DocumentMatch { id, path, content_matches } => {
                        let file = &core.get_file_by_id(*id).unwrap();
                        Self::show_file(ui, file, path);
                        ui.horizontal(|ui| {
                            ui.add_space(15.0);
                            ui.horizontal_wrapped(|ui| {
                                let font_size = 15.0;
                                self.show_content_match(ui, &content_matches[0], font_size);
                            });
                        });
                    }
                    SearchResult::PathMatch { id, path, matched_indices: _, score: _ } => {
                        let file = &core.get_file_by_id(*id).unwrap();
                        Self::show_file(ui, file, path);
                    }
                };
                ui.add_space(10.0);
            });
            ui.add(egui::Separator::default().shrink(ui.available_width() / 1.5));
            ui.add_space(10.0);

            let sr_res = ui.interact(sr_res.response.rect, ui.next_auto_id(), egui::Sense::click());
            if sr_res.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand)
            }

            if sr_res.clicked() {
                return Some(sr.id());
            };
        }

        None
    }

    fn show_file(ui: &mut egui::Ui, file: &File, path: &str) {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(Self::X_MARGIN);

            DocType::from_name(file.name.as_str()).to_icon().show(ui);

            ui.add_space(7.0);

            let mut job = egui::text::LayoutJob::single_section(
                file.name.clone(),
                egui::TextFormat::simple(
                    egui::FontId::proportional(18.0),
                    ui.visuals().text_color(),
                ),
            );
            job.wrap = egui::epaint::text::TextWrapping {
                overflow_character: Some('…'),
                max_rows: 1,
                break_anywhere: true,
                ..Default::default()
            };
            ui.label(job);
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(Self::X_MARGIN);

            let mut job = egui::text::LayoutJob::single_section(
                path.to_owned(),
                egui::TextFormat::simple(egui::FontId::proportional(15.0), egui::Color32::GRAY),
            );

            job.wrap = egui::epaint::text::TextWrapping {
                overflow_character: Some('…'),
                max_rows: 1,
                break_anywhere: true,
                ..Default::default()
            };
            ui.label(job);
        });
    }

    fn show_content_match(&self, ui: &mut egui::Ui, content_match: &ContentMatch, font_size: f32) {
        let matched_indices = &content_match.matched_indices;
        let str = content_match.paragraph.clone();
        let highlight_color = ui.visuals().widgets.active.bg_fill.gamma_multiply(0.5);

        let mut curr = 0;
        let mut next;

        let pre = str[0..matched_indices[0]].to_string();
        ui.label(egui::RichText::new(pre).size(font_size));

        while curr < matched_indices.len() {
            next = curr;

            while next < matched_indices.len() - 1
                && matched_indices[next] + 1 == matched_indices[next + 1]
            {
                next += 1;
            }

            if next == curr || curr == matched_indices.len() - 1 {
                let h_str = str
                    .chars()
                    .nth(matched_indices[curr])
                    .unwrap_or_default()
                    .to_string();
                ui.label(
                    egui::RichText::new(h_str)
                        .size(font_size)
                        .background_color(highlight_color),
                );

                curr += 1;
            } else {
                let h_str = str[matched_indices[curr]..matched_indices[next] + 1].to_string();

                ui.label(
                    egui::RichText::new(h_str)
                        .size(font_size)
                        .background_color(highlight_color),
                );
                curr = next + 1;
            }
            if curr < matched_indices.len() - 1 {
                ui.label(
                    egui::RichText::new(
                        str[matched_indices[next] + 1..matched_indices[curr]].to_string(),
                    )
                    .size(font_size),
                );
            }
        }

        let post = str[matched_indices[matched_indices.len() - 1] + 1..].to_string();
        ui.label(egui::RichText::new(post).size(font_size));
    }
}
